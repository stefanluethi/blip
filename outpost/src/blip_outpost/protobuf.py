"""Translates the protobuf messages of a target to blip, as the annotations of the schema
say (`blip/options.proto`), and offers the actions of its downlink message to blip.

    schema = Schema.load(["link.proto"])
    translator = Translator(schema, "link.Uplink", outpost)
    translator.feed(payload)                    # one Uplink message, serialized

    invoker = Invoker(schema, "link.Downlink", send=link.send)
    invoker.install(outpost.actions)            # one action per member of the Invoke oneof

The rules, in short (see doc/source/usage/outpost.rst):

- A message has the path of its annotation, else the path of the field that holds it;
  the uplink message has the path "".
- A value field is a trace at the path of its message joined with its name, or with the
  name of its annotation; with `record`, the value fields of a message are one record
  value at the path of the message.
- A message's timestamp field times its values and those of the messages in it; without
  one, the outpost estimates the time of the target from the latest timestamp.
- With a period, the repeated fields are series: one value per sample.
"""

import asyncio
import logging
import math
import time
from dataclasses import dataclass
from typing import Any, Awaitable, Callable, Dict, List, Optional, Tuple

from google.protobuf.message import DecodeError

from blip_outpost.actions import ActionError, Actions, Param
from blip_outpost.schema import (
    BITS_64,
    FD,
    FLOAT_TYPES,
    INTEGER_TYPES,
    KIND_ACTION_RESULT,
    KIND_INVOKE,
    KIND_LOG,
    UNSIGNED_TYPES,
    FieldPlan,
    MessagePlan,
    Schema,
    SchemaError,
    join,
)

logger = logging.getLogger(__name__)

# blip's log levels go up to ERROR
MAX_LEVEL = 4
DEFAULT_LEVEL = 2


def wrap(f: FieldPlan) -> int:
    """The modulus of a counter in field `f`."""
    bits = 64 if f.desc.type in BITS_64 else 32
    if f.desc.type not in UNSIGNED_TYPES:
        bits -= 1
    return 1 << bits


class Translator:
    """Turns serialized messages of the type `uplink` into blip messages, sent to
    `outpost`. Answers to actions go to `on_action_result(id, error)`.

    Counts the messages and their problems in `counters`: `frames`, `decode_errors`,
    and with a sequence field `seq_gaps` and `resets`."""

    def __init__(
        self,
        schema: Schema,
        uplink: str,
        outpost: Any,
        on_action_result: Optional[Callable[[int, str], None]] = None,
        clock: Callable[[], int] = time.monotonic_ns,
        wall_clock: Callable[[], int] = time.time_ns,
    ) -> None:
        self.schema = schema
        self.outpost = outpost
        self.on_action_result = on_action_result
        self._class = schema.message_class(uplink)
        self._root = schema.plan(uplink)
        # reads every annotation now, so mistakes show at the start
        self.plans = list(schema.reachable(uplink))
        self.has_action_results = any(p.kind == KIND_ACTION_RESULT for p in self.plans)
        self._clock = clock
        self._wall_clock = wall_clock

        self.counters: Dict[str, int] = {"frames": 0, "decode_errors": 0}
        if self._root.sequence is not None:
            self.counters.update(seq_gaps=0, resets=0)
        self._expected_seq: Optional[int] = None
        # the newest time of the target, and the clock when it arrived
        self._last_time: Optional[Tuple[int, int]] = None
        # paths whose unit was sent
        self._units: set = set()
        self._warned: set = set()

    # Time

    def target_time_ns(self) -> int:
        """The estimated time of the target now: its newest timestamp plus the time
        since; the time of this computer (since the Unix epoch) before the first."""
        if self._last_time is None:
            return self._wall_clock()
        t, at = self._last_time
        return t + (self._clock() - at)

    def _seen(self, t: int) -> None:
        self._last_time = (t, self._clock())

    # Messages

    def restart(self) -> None:
        """The connection to the target was lost: the next sequence number starts over
        without counting a gap."""
        self._expected_seq = None

    def feed(self, payload: bytes) -> None:
        """Translates one serialized uplink message."""
        try:
            message = self._class.FromString(payload)
        except DecodeError as e:
            self.counters["decode_errors"] += 1
            logger.debug(f"Dropping {len(payload)} bytes that are no message ({e}): {payload[:32].hex()}")
            return
        self.counters["frames"] += 1
        if self._root.sequence is not None:
            self._check_sequence(getattr(message, self._root.sequence.name), wrap(self._root.sequence))
        try:
            self._walk(message, self._root, "", self.target_time_ns())
        except Exception:
            self.counters["decode_errors"] += 1
            logger.exception("Failed to translate a message")

    def _check_sequence(self, seq: int, modulus: int) -> None:
        expected = self._expected_seq
        self._expected_seq = (seq + 1) % modulus
        if expected is None:
            return
        d = (seq - expected) % modulus
        if d >= modulus // 2:
            d -= modulus
        if d == 0:
            return
        if d < 0 or seq == 0:
            self.counters["resets"] += 1
        else:
            self.counters["seq_gaps"] += d

    def _walk(self, message: Any, plan: MessagePlan, parent_path: str, t: int) -> None:
        path = self._render(plan.template, message) if plan.template is not None else parent_path
        if plan.timestamp is not None:
            t = self._timestamp(message, plan.timestamp)

        if plan.kind == KIND_LOG:
            self._log(message, plan, t)
        elif plan.kind == KIND_ACTION_RESULT:
            self._action_result(message, plan)
        elif plan.kind != KIND_INVOKE and plan.values:
            if plan.period is not None:
                self._series(message, plan, path, t)
            else:
                self._values(message, plan, path, t)

        for child in plan.children:
            if child.repeated:
                items = getattr(message, child.name)
                if not items:
                    continue
            elif message.HasField(child.name):
                items = (getattr(message, child.name),)
            else:
                continue
            child_path = self._field_path(child, message, path)
            child_plan = self.schema.plan(child.message_name)
            for item in items:
                self._walk(item, child_plan, child_path, t)

    def _timestamp(self, message: Any, f: FieldPlan) -> int:
        raw = getattr(message, f.name)
        t = int(round(raw * f.scale)) if f.scale is not None else int(raw)
        self._seen(t)
        return t

    def _render(self, template: List[Tuple[str, Optional[str]]], message: Any) -> str:
        parts = []
        for literal, name in template:
            parts.append(literal)
            if name is not None:
                value = getattr(message, name)
                f = message.DESCRIPTOR.fields_by_name[name]
                if f.type == FD.TYPE_ENUM:
                    value = self.schema.enum_names(f.enum_type).get(value, value)
                parts.append(str(value))
        return join("".join(parts))

    def _field_path(self, f: FieldPlan, message: Any, path: str) -> str:
        if f.path is not None:
            return self._render(f.path, message)
        return join(path, f.key)

    def _unit(self, path: str, unit: str) -> None:
        if unit and path not in self._units:
            self._units.add(path)
            self.outpost.send_metadata(path, unit)

    def _convert(self, f: FieldPlan, raw: Any) -> Any:
        if not f.repeated:
            return self._scalar(f, raw)
        values = [self._scalar(f, v) for v in raw]
        if f.shape:
            return self._reshape(f, values)
        return values

    def _scalar(self, f: FieldPlan, raw: Any) -> Any:
        if f.null_below is not None and raw < f.null_below:
            return None
        if f.enum_names is not None:
            return f.enum_names.get(raw, str(raw))
        if f.scale is not None:
            return raw * f.scale
        return raw

    def _reshape(self, f: FieldPlan, values: List[Any]) -> Any:
        if math.prod(f.shape) != len(values):
            self._warn_once(
                ("shape", f.desc.full_name),
                f"{f.desc.full_name}: {len(values)} values do not fit the shape {list(f.shape)}, sent flat",
            )
            return values
        for length in reversed(f.shape[1:]):
            values = [values[i:i + length] for i in range(0, len(values), length)]
        return values

    def _present(self, message: Any, plan: MessagePlan) -> List[FieldPlan]:
        return [f for f in plan.values if not f.has_presence or message.HasField(f.name)]

    def _values(self, message: Any, plan: MessagePlan, path: str, t: int) -> None:
        fields = self._present(message, plan)
        if plan.record:
            if not path:
                self._warn_once(("record", plan.full_name), f"{plan.full_name}: a record needs a path")
                return
            for f in fields:
                self._unit(f"{path}.{f.key}", f.unit)
            self.outpost.send_value(path, {f.key: self._convert(f, getattr(message, f.name)) for f in fields}, t)
            return
        for f in fields:
            field_path = self._field_path(f, message, path)
            if not field_path:
                continue
            self._unit(field_path, f.unit)
            self.outpost.send_value(field_path, self._convert(f, getattr(message, f.name)), t)

    def _series(self, message: Any, plan: MessagePlan, path: str, t: int) -> None:
        period_field = plan.period
        assert period_field is not None
        raw_period = getattr(message, period_field.name)
        period = raw_period * period_field.scale if period_field.scale is not None else raw_period
        fields = self._present(message, plan)
        columns = [(f, [self._scalar(f, v) for v in getattr(message, f.name)]) for f in fields if f.repeated]
        constants = [(f, self._convert(f, getattr(message, f.name))) for f in fields if not f.repeated]
        n = max((len(values) for _, values in columns), default=0)
        if n == 0:
            return
        send = self.outpost.send_value

        if plan.record:
            if not path:
                self._warn_once(("record", plan.full_name), f"{plan.full_name}: a record needs a path")
                return
            for f in fields:
                self._unit(f"{path}.{f.key}", f.unit)
            for i in range(n):
                record = {f.key: values[i] if i < len(values) else None for f, values in columns}
                for f, value in constants:
                    record[f.key] = value
                send(path, record, t + int(round(i * period)))
        else:
            for f, value in constants:
                field_path = self._field_path(f, message, path)
                self._unit(field_path, f.unit)
                send(field_path, value, t)
            for f, values in columns:
                field_path = self._field_path(f, message, path)
                self._unit(field_path, f.unit)
                for i, value in enumerate(values):
                    send(field_path, value, t + int(round(i * period)))
        if plan.timestamp is not None:
            self._seen(t + int(round((n - 1) * period)))

    def _log(self, message: Any, plan: MessagePlan, t: int) -> None:
        level = DEFAULT_LEVEL
        if plan.level is not None:
            level = min(max(int(getattr(message, plan.level.name)), 0), MAX_LEVEL)
        component = ""
        if plan.component is not None:
            f = plan.component
            raw = getattr(message, f.name)
            component = f.enum_names.get(raw, str(raw)) if f.enum_names is not None else str(raw)
        text = str(getattr(message, plan.message.name)) if plan.message is not None else ""
        self.outpost.send_log(level, text, component, t)

    def _action_result(self, message: Any, plan: MessagePlan) -> None:
        if plan.action_id is None:
            self._warn_once(("result", plan.full_name), f"{plan.full_name}: an action result needs an id")
            return
        error = str(getattr(message, plan.error.name)) if plan.error is not None else ""
        if self.on_action_result is not None:
            self.on_action_result(int(getattr(message, plan.action_id.name)), error)

    def _warn_once(self, key: Any, text: str) -> None:
        if key not in self._warned:
            self._warned.add(key)
            logger.warning(text)


@dataclass
class ActionPlan:
    """An action of the downlink message: a member of the oneof of the Invoke message."""

    path: str
    member: FieldPlan
    params: List[Param]
    # the number of each enum parameter's option
    enum_numbers: Dict[str, Dict[str, int]]


class Invoker:
    """The actions of the downlink message type `downlink`: one per message member of
    the oneof of its KIND_INVOKE message, with the fields of the member as parameters.

    An invocation sends a downlink message with `send` and, if `wait_for_results`, waits
    `timeout` seconds for the answer of the target, see `resolve`."""

    def __init__(
        self,
        schema: Schema,
        downlink: str,
        send: Callable[[bytes], Awaitable[None]],
        wait_for_results: bool = True,
        timeout: float = 1.0,
    ) -> None:
        self.schema = schema
        self._class = schema.message_class(downlink)
        self._send = send
        self.wait_for_results = wait_for_results
        self.timeout = timeout

        root = schema.plan(downlink)
        self._sequence = root.sequence
        self._seq = 0
        self._chain: List[str] = []
        self._id_field: Optional[FieldPlan] = None
        self._next_id = 1
        self._pending: Dict[int, asyncio.Future] = {}
        self.actions: List[ActionPlan] = []

        found = self._find_invoke(downlink)
        if found is None:
            logger.warning(f"{downlink} holds no message of kind KIND_INVOKE, so there are no actions")
            return
        self._chain, invoke = found
        self._id_field = invoke.action_id
        for f in sorted(invoke.desc.fields, key=lambda f: f.number):
            if f.containing_oneof is None or f.type != FD.TYPE_MESSAGE:
                continue
            member = schema._field_plan(invoke.desc, f)
            if member.skip:
                continue
            action = self._action(member)
            if action is not None:
                self.actions.append(action)

    def _find_invoke(self, downlink: str) -> Optional[Tuple[List[str], MessagePlan]]:
        """The fields from the downlink message to its KIND_INVOKE message, and its plan."""
        todo: List[Tuple[List[str], str]] = [([], downlink)]
        seen = set()
        while todo:
            chain, name = todo.pop(0)
            if name in seen:
                continue
            seen.add(name)
            plan = self.schema.plan(name)
            if plan.kind == KIND_INVOKE:
                return chain, plan
            for child in plan.children:
                if not child.repeated:
                    todo.append((chain + [child.name], child.message_name))
        return None

    def _action(self, member: FieldPlan) -> Optional[ActionPlan]:
        path = "".join(literal for literal, _ in member.path) if member.path is not None else member.name
        params = []
        enum_numbers = {}
        plan = self.schema.plan(member.message_name)
        for f in plan.values + plan.children:
            param = self._param(f)
            if param is None:
                logger.warning(f"Action '{path}': parameter '{f.name}' has an unsupported type, the action is left out")
                return None
            params.append(param)
            if f.enum_names is not None:
                enum_numbers[f.name] = {name: number for number, name in f.enum_names.items()}
        return ActionPlan(path=path, member=member, params=params, enum_numbers=enum_numbers)

    def _param(self, f: FieldPlan) -> Optional[Param]:
        if f.repeated or f.is_message:
            return None
        t = f.desc.type
        param = Param(name=f.name, kind="", unit=f.unit, required=f.default is None)
        if t in INTEGER_TYPES:
            param.kind = "Int"
            param.min = int(f.min) if f.min is not None else (0 if t in UNSIGNED_TYPES else None)
            param.max = int(f.max) if f.max is not None else None
            param.default = int(f.default) if f.default is not None else None
        elif t in FLOAT_TYPES:
            param.kind = "Float"
            param.min, param.max, param.step, param.default = f.min, f.max, f.step, f.default
        elif t == FD.TYPE_BOOL:
            param.kind = "Bool"
            param.default = bool(f.default) if f.default is not None else None
        elif t == FD.TYPE_STRING:
            param.kind = "String"
            param.required = True
        elif t == FD.TYPE_ENUM and f.enum_names is not None:
            param.kind = "Enum"
            param.options = list(f.enum_names.values())
            param.default = f.enum_names.get(int(f.default)) if f.default is not None else None
            param.required = param.default is None
        else:
            return None
        return param

    def install(self, actions: Actions) -> None:
        """Adds the actions to `actions`, e.g. `outpost.actions`, announced once."""
        with actions.batch():
            for action in self.actions:
                actions.add(
                    action.path,
                    self._function(action),
                    description=action.member.description,
                    readback=action.member.readback or None,
                    confirm=action.member.confirm,
                    params=action.params,
                )

    def _function(self, action: ActionPlan) -> Callable[..., Awaitable[None]]:
        async def invoke(**kwargs: Any) -> None:
            await self.invoke(action, kwargs)

        invoke.__name__ = action.member.name
        return invoke

    def encode(self, action: ActionPlan, args: Dict[str, Any]) -> Tuple[int, bytes]:
        """The id and the bytes of the downlink message invoking `action` with `args`."""
        message = self._class()
        if self._sequence is not None:
            setattr(message, self._sequence.name, self._seq)
            self._seq = (self._seq + 1) % wrap(self._sequence)
        target = message
        for name in self._chain:
            target = getattr(target, name)
        id = self._next_id
        if self._id_field is not None:
            self._next_id = id % (wrap(self._id_field) - 1) + 1
            setattr(target, self._id_field.name, id)
        member = getattr(target, action.member.name)
        member.SetInParent()
        for name, value in args.items():
            numbers = action.enum_numbers.get(name)
            if numbers is not None:
                value = numbers[value]
            try:
                setattr(member, name, value)
            except (TypeError, ValueError) as e:
                raise ActionError(f"{name}: {e}") from None
        return id, message.SerializeToString()

    async def invoke(self, action: ActionPlan, args: Dict[str, Any]) -> None:
        """Sends the action to the target and waits for its answer, raises ActionError
        if the target rejects it or does not answer in time."""
        id, payload = self.encode(action, args)
        wait = self.wait_for_results and self._id_field is not None
        if wait:
            # an older invocation of the same id is not answered any more
            future = asyncio.get_running_loop().create_future()
            self._pending[id] = future
        try:
            await self._send(payload)
            if not wait:
                return
            try:
                error = await asyncio.wait_for(future, self.timeout)
            except asyncio.TimeoutError:
                raise ActionError("no answer from the target") from None
        finally:
            if wait and self._pending.get(id) is future:
                del self._pending[id]
        if error:
            raise ActionError(error)

    def resolve(self, id: int, error: str) -> None:
        """The answer of the target to the invocation `id`: empty `error` if it was
        carried out. An answer nobody waits for is dropped."""
        future = self._pending.get(id)
        if future is not None and not future.done():
            future.set_result(error)

    def disconnected(self) -> None:
        """Rejects the invocations that wait for an answer: the link is down."""
        for future in self._pending.values():
            if not future.done():
                future.set_exception(ActionError("the connection to the target was lost"))


__all__ = ["ActionPlan", "Invoker", "SchemaError", "Translator"]
