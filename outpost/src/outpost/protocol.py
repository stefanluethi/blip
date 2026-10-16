"""The base class of protocols, which connect the outpost to a target."""

import asyncio
import logging
from typing import Any, Awaitable, Callable, Optional, Set, Union

from outpost.actions import Actions, declared_actions
from outpost.proto import Data, Message

logger = logging.getLogger("outpost.protocol")

SendCallback = Callable[[bytes], Awaitable[None]]


class Protocol:
    """Connects the outpost to a target: translates what the target sends to blip
    messages and carries out the actions blip invokes.

    A protocol implements `run`, which the outpost runs while it serves and cancels
    when it shuts down. Methods decorated with `@action` are its actions, more can be
    added at runtime through `actions`. The keyword arguments of the constructor are
    the options set with ``--option`` on the command line, converted by their type
    hints.
    """

    _actions: Optional[Actions] = None
    _send_callback: Optional[SendCallback] = None

    @property
    def actions(self) -> Actions:
        """The actions offered to blip, starting with the methods decorated with
        @action. Changing them announces them to blip again."""
        if self._actions is None:
            actions = Actions()
            for info, method in declared_actions(self):
                actions.add(
                    info.path,
                    method,
                    description=info.description,
                    readback=info.readback,
                    confirm=info.confirm,
                )
            # only now, so the declared actions are announced once, by the outpost
            actions.on_change = self._actions_changed
            self._actions = actions
        return self._actions

    async def run(self) -> None:
        """Talks to the target until cancelled. Returning ends the protocol but not the
        outpost, raising an exception stops the outpost."""
        raise NotImplementedError

    async def on_client_connected(self) -> None:
        """Called when blip connects, after the actions were sent to it."""

    async def invoke(self, path: str, args: dict) -> None:
        """Carries out the action at `path` with the arguments `args` by parameter
        name, raises ActionError to reject it. Calls the registered action, override it
        to forward invocations to a target that knows the actions itself."""
        await self.actions.invoke(path, args)

    async def send(self, message: Union[Message, bytes]) -> None:
        """Sends a message, or the bytes of one, to blip."""
        if self._send_callback is None:
            return
        frame = message.encode_cbor() if isinstance(message, Message) else message
        try:
            await self._send_callback(frame)
        except Exception as e:
            logger.error(f"Error sending to blip: {e}")

    async def send_value(self, path: str, value: Data, timestamp_ns: Optional[int] = None) -> None:
        """Sends a trace sample, timestamped now unless given."""
        await self.send(Message.create_value(path, value, timestamp_ns))

    async def send_metadata(self, path: str, unit: str) -> None:
        await self.send(Message.create_metadata(path, unit))

    async def send_log(
        self,
        level: int,
        message: str,
        component: str = "",
        timestamp_ns: Optional[int] = None,
    ) -> None:
        """Sends a log record of a `LogLevel`, timestamped now unless given."""
        await self.send(Message.create_log(int(level), message, timestamp_ns, component))

    async def announce_actions(self) -> None:
        """Sends the list of actions to blip, done after each change of `actions`."""
        await self.send(Message.create_actions(self.actions.descriptors()))

    def attach(self, send: Optional[SendCallback]) -> None:
        """Sets where `send` sends messages to, called by the outpost."""
        self._send_callback = send

    _announcing: Set["asyncio.Task[Any]"] = set()

    def _actions_changed(self) -> None:
        if self._send_callback is None:
            return
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            return
        task = loop.create_task(self.announce_actions())
        # keep a reference, so the task is not collected while it runs
        Protocol._announcing.add(task)
        task.add_done_callback(Protocol._announcing.discard)
