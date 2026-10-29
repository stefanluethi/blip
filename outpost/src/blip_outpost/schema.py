"""Loads a protobuf schema at run time and reads its blip annotations
(`blip/options.proto`), so the outpost needs no code generated from the schema.

    schema = Schema.load(["robby_link.proto"], include_dirs=["proto"])
    plan = schema.plan("robby.link.Uplink")

protoc compiles the .proto files to descriptors; the message classes are built from them
with the protobuf runtime. A `MessagePlan` is what the outpost does with a message type,
read once from the annotations.
"""

import re
import shutil
import string
import subprocess
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional, Sequence, Tuple, Union

from google.protobuf import descriptor, descriptor_pb2, descriptor_pool, message_factory

# Holds blip/options.proto
PROTO_DIR = Path(__file__).resolve().parent / "proto"
OPTIONS_FILE = "blip/options.proto"

# The enums of blip/options.proto, by number
KIND_VALUES, KIND_LOG, KIND_ACTION_RESULT, KIND_INVOKE = range(4)
(
    ROLE_VALUE,
    ROLE_TIMESTAMP,
    ROLE_PERIOD,
    ROLE_SEQUENCE,
    ROLE_ACTION_ID,
    ROLE_ERROR,
    ROLE_LEVEL,
    ROLE_COMPONENT,
    ROLE_MESSAGE,
) = range(9)

FD = descriptor.FieldDescriptor
INTEGER_TYPES = {
    FD.TYPE_INT32, FD.TYPE_INT64, FD.TYPE_UINT32, FD.TYPE_UINT64, FD.TYPE_SINT32,
    FD.TYPE_SINT64, FD.TYPE_FIXED32, FD.TYPE_FIXED64, FD.TYPE_SFIXED32, FD.TYPE_SFIXED64,
}
UNSIGNED_TYPES = {FD.TYPE_UINT32, FD.TYPE_UINT64, FD.TYPE_FIXED32, FD.TYPE_FIXED64}
FLOAT_TYPES = {FD.TYPE_FLOAT, FD.TYPE_DOUBLE}
BITS_64 = {
    FD.TYPE_INT64, FD.TYPE_UINT64, FD.TYPE_SINT64, FD.TYPE_FIXED64, FD.TYPE_SFIXED64,
}

PathLike = Union[str, Path]


def is_repeated(f: descriptor.FieldDescriptor) -> bool:
    """protobuf 7 has `is_repeated`, older versions only `label`."""
    repeated = getattr(f, "is_repeated", None)
    if repeated is not None:
        return bool(repeated)
    return f.label == FD.LABEL_REPEATED


class SchemaError(Exception):
    """The schema does not compile, or its annotations do not fit together."""


def compile_schema(
    protos: Sequence[PathLike],
    include_dirs: Sequence[PathLike] = (),
    protoc: str = "protoc",
) -> descriptor_pb2.FileDescriptorSet:
    """The descriptors of `protos` and of everything they import, compiled by `protoc`.

    The outpost's own `blip/options.proto` comes first on the import path, so a schema
    may import it without a copy of its own; an older copy next to the schema is not
    used. A .proto that is in none of `include_dirs` is found in its own directory."""
    executable = shutil.which(protoc)
    if executable is None:
        raise SchemaError(f"'{protoc}' not found: blip-outpost protobuf needs protoc on the PATH")
    includes = [PROTO_DIR] + [Path(d).resolve() for d in include_dirs]
    for proto in protos:
        parent = Path(proto).resolve().parent
        if not any(parent == d or d in parent.parents for d in includes[1:]):
            includes.append(parent)

    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "schema.pb"
        command = [
            executable,
            "--include_imports",
            f"--descriptor_set_out={out}",
            *(f"-I{d}" for d in includes),
            # so the annotations can be read also of a schema that does not use them
            str(PROTO_DIR / OPTIONS_FILE),
            *(str(Path(p).resolve()) for p in protos),
        ]
        result = subprocess.run(command, capture_output=True, text=True)
        if result.returncode != 0:
            raise SchemaError(f"protoc failed:\n{result.stderr.strip()}")
        return descriptor_pb2.FileDescriptorSet.FromString(out.read_bytes())


def upper_snake(name: str) -> str:
    """`LogLevel` → `LOG_LEVEL`."""
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name).upper()


def join(*parts: str) -> str:
    """The path of the levels that are not empty."""
    return "/".join(p.strip("/") for p in parts if p and p.strip("/"))


@dataclass
class FieldPlan:
    """What the outpost does with a field."""

    desc: descriptor.FieldDescriptor
    name: str
    # the level of the path, or the key in a record
    key: str
    # the whole path instead, a template of the fields of the message
    path: Optional[List[Tuple[str, Optional[str]]]]
    role: int
    unit: str
    scale: Optional[float]
    null_below: Optional[float]
    shape: Tuple[int, ...]
    skip: bool
    repeated: bool
    is_message: bool
    # whether a value is only there when set: a member of a oneof, or `optional`
    has_presence: bool
    # an enum field: the blip name of each number
    enum_names: Optional[Dict[int, str]]
    # actions and their parameters
    description: str = ""
    confirm: bool = False
    readback: str = ""
    min: Optional[float] = None
    max: Optional[float] = None
    default: Optional[float] = None
    step: Optional[float] = None

    @property
    def message_type(self) -> Optional[descriptor.Descriptor]:
        return self.desc.message_type

    @property
    def message_name(self) -> str:
        """The full name of the message type of a message field."""
        if self.desc.message_type is None:
            raise SchemaError(f"{self.desc.full_name} holds no message")
        return self.desc.message_type.full_name


@dataclass
class MessagePlan:
    """What the outpost does with a message type."""

    full_name: str
    desc: descriptor.Descriptor
    kind: int
    path: Optional[str]
    record: bool
    # the fields the path template names, as (literal, field) pieces
    template: Optional[List[Tuple[str, Optional[str]]]]
    timestamp: Optional[FieldPlan] = None
    period: Optional[FieldPlan] = None
    sequence: Optional[FieldPlan] = None
    action_id: Optional[FieldPlan] = None
    error: Optional[FieldPlan] = None
    level: Optional[FieldPlan] = None
    component: Optional[FieldPlan] = None
    message: Optional[FieldPlan] = None
    # the scalar fields with a value for blip, in field number order
    values: List[FieldPlan] = field(default_factory=list)
    # the fields that hold messages
    children: List[FieldPlan] = field(default_factory=list)

    def fixed_path(self) -> bool:
        return self.template is None or all(name is None for _, name in self.template)


class Schema:
    """The message types of a compiled schema and their blip annotations."""

    def __init__(self, files: descriptor_pb2.FileDescriptorSet) -> None:
        self.pool = descriptor_pool.DescriptorPool()
        names = []
        for file in files.file:
            # the same file may come twice, e.g. options.proto as input and as import
            if file.name in names:
                continue
            self.pool.Add(file)
            names.append(file.name)
        self._classes = message_factory.GetMessageClassesForFiles(names, self.pool)

        def extension(name: str) -> Any:
            return self.pool.FindExtensionByName(name)

        self._options = {
            "message": (self._classes["google.protobuf.MessageOptions"], extension("blip.message")),
            "field": (self._classes["google.protobuf.FieldOptions"], extension("blip.field")),
            "value": (self._classes["google.protobuf.EnumValueOptions"], extension("blip.value")),
        }
        # the serialized options of each message, field and enum value, as protoc wrote them
        self._raw: Dict[Tuple[str, ...], bytes] = {}
        for file in files.file:
            prefix = f"{file.package}." if file.package else ""
            for message in file.message_type:
                self._read_message(prefix, message)
            for enum in file.enum_type:
                self._read_enum(prefix, enum)
        self._plans: Dict[str, MessagePlan] = {}

    @classmethod
    def load(
        cls,
        protos: Sequence[PathLike],
        include_dirs: Sequence[PathLike] = (),
        protoc: str = "protoc",
    ) -> "Schema":
        """Compiles `protos` with protoc, see `compile_schema`."""
        return cls(compile_schema(protos, include_dirs, protoc))

    def _read_message(self, prefix: str, message: descriptor_pb2.DescriptorProto) -> None:
        name = prefix + message.name
        self._raw[("message", name)] = message.options.SerializeToString()
        for f in message.field:
            self._raw[("field", name, f.name)] = f.options.SerializeToString()
        for nested in message.nested_type:
            self._read_message(f"{name}.", nested)
        for enum in message.enum_type:
            self._read_enum(f"{name}.", enum)

    def _read_enum(self, prefix: str, enum: descriptor_pb2.EnumDescriptorProto) -> None:
        name = prefix + enum.name
        for value in enum.value:
            self._raw[("value", name, str(value.number))] = value.options.SerializeToString()

    def options(self, what: str, *key: str) -> Any:
        """The blip options of a message, field or enum value (defaults if none)."""
        options_class, extension = self._options[what]
        options = options_class.FromString(self._raw.get((what, *key), b""))
        return options.Extensions[extension]

    def message_class(self, full_name: str) -> Any:
        """The class of the message type `full_name`, e.g. `robby.link.Uplink`."""
        try:
            return self._classes[full_name]
        except KeyError:
            raise SchemaError(f"no message type '{full_name}' in the schema") from None

    def find(self, full_name: str) -> descriptor.Descriptor:
        try:
            return self.pool.FindMessageTypeByName(full_name)
        except KeyError:
            raise SchemaError(f"no message type '{full_name}' in the schema") from None

    def enum_names(self, enum: descriptor.EnumDescriptor) -> Dict[int, str]:
        """The blip name of each value of `enum`: its annotation, or its name without
        the prefix of the enum, in lower case."""
        prefix = upper_snake(enum.name) + "_"
        names = {}
        for value in enum.values:
            options = self.options("value", enum.full_name, str(value.number))
            if options.HasField("name"):
                names[value.number] = options.name
            else:
                bare = value.name[len(prefix):] if value.name.startswith(prefix) else value.name
                names[value.number] = bare.lower()
        return names

    def plan(self, full_name: str) -> MessagePlan:
        """What the outpost does with the message type `full_name`."""
        plan = self._plans.get(full_name)
        if plan is None:
            plan = self._make_plan(self.find(full_name))
            self._plans[full_name] = plan
        return plan

    def _make_plan(self, desc: descriptor.Descriptor) -> MessagePlan:
        options = self.options("message", desc.full_name)
        template = None
        if options.HasField("path"):
            template = parse_template(options.path, desc)
        plan = MessagePlan(
            full_name=desc.full_name,
            desc=desc,
            kind=options.kind,
            path=options.path if options.HasField("path") else None,
            record=options.record,
            template=template,
        )
        in_template = {name for _, name in template or [] if name is not None}

        roles = {
            ROLE_TIMESTAMP: "timestamp",
            ROLE_PERIOD: "period",
            ROLE_SEQUENCE: "sequence",
            ROLE_ACTION_ID: "action_id",
            ROLE_ERROR: "error",
            ROLE_LEVEL: "level",
            ROLE_COMPONENT: "component",
            ROLE_MESSAGE: "message",
        }
        for f in sorted(desc.fields, key=lambda f: f.number):
            field_plan = self._field_plan(desc, f)
            if field_plan.role != ROLE_VALUE:
                attribute = roles.get(field_plan.role)
                if attribute is None:
                    raise SchemaError(f"{desc.full_name}.{f.name}: unknown role {field_plan.role}")
                if getattr(plan, attribute) is not None:
                    raise SchemaError(f"{desc.full_name}: two fields of the role {attribute}")
                setattr(plan, attribute, field_plan)
                continue
            if field_plan.skip or f.name in in_template:
                continue
            if field_plan.is_message:
                plan.children.append(field_plan)
            elif f.type != FD.TYPE_BYTES:
                plan.values.append(field_plan)
        if plan.period is not None and plan.timestamp is None:
            raise SchemaError(f"{desc.full_name}: a period needs a timestamp")
        return plan

    def _field_plan(self, desc: descriptor.Descriptor, f: descriptor.FieldDescriptor) -> FieldPlan:
        options = self.options("field", desc.full_name, f.name)
        repeated = is_repeated(f)
        shape = tuple(options.shape)
        if shape and not repeated:
            raise SchemaError(f"{desc.full_name}.{f.name}: only a repeated field has a shape")
        return FieldPlan(
            desc=f,
            name=f.name,
            key=options.name if options.HasField("name") else f.name,
            path=parse_template(options.path, desc) if options.HasField("path") else None,
            role=options.role,
            unit=options.unit,
            scale=options.scale if options.HasField("scale") else None,
            null_below=options.null_below if options.HasField("null_below") else None,
            shape=shape,
            skip=options.skip,
            repeated=repeated,
            is_message=f.type == FD.TYPE_MESSAGE,
            has_presence=f.has_presence and not repeated,
            enum_names=self.enum_names(f.enum_type) if f.type == FD.TYPE_ENUM else None,
            description=options.description,
            confirm=options.confirm,
            readback=options.readback,
            min=options.min if options.HasField("min") else None,
            max=options.max if options.HasField("max") else None,
            default=options.default if options.HasField("default") else None,
            step=options.step if options.HasField("step") else None,
        )

    def reachable(self, full_name: str) -> Iterable[MessagePlan]:
        """The plans of `full_name` and of all message types in it, each once."""
        seen = set()
        todo = [full_name]
        while todo:
            name = todo.pop()
            if name in seen:
                continue
            seen.add(name)
            plan = self.plan(name)
            yield plan
            for child in plan.children:
                todo.append(child.message_name)


def parse_template(path: str, desc: descriptor.Descriptor) -> List[Tuple[str, Optional[str]]]:
    """The pieces of a path template: literal text, then the field whose value follows
    (None at the end). Raises SchemaError for a field the message does not have."""
    pieces = []
    try:
        parsed = list(string.Formatter().parse(path))
    except ValueError as e:
        raise SchemaError(f"{desc.full_name}: path '{path}': {e}") from None
    for literal, name, spec, conversion in parsed:
        if name is not None:
            f = desc.fields_by_name.get(name)
            if f is None:
                raise SchemaError(f"{desc.full_name}: path '{path}' names no field '{name}'")
            if is_repeated(f) or f.type == FD.TYPE_MESSAGE:
                raise SchemaError(f"{desc.full_name}: path '{path}': '{name}' is no single value")
            if spec or conversion:
                raise SchemaError(f"{desc.full_name}: path '{path}': no format specs")
        pieces.append((literal, name))
    return pieces
