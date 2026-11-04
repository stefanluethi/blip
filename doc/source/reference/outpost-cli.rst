outpost command line
====================

The ``blip-outpost`` command serves targets that need no script of their own:
those that speak the blip protocol themselves (``tcp``, ``udp``), those that send
protobuf messages of an annotated schema (``protobuf``), and recordings
(``replay``). ``blip-outpost --help`` and ``blip-outpost COMMAND --help`` print
the same options. To connect a target of your own in Python, see
:doc:`/outpost/index`.

A test (``outpost/tests/test_docs.py``) fails when an option of the command is
missing on this page.

Options of every command
------------------------

Each command takes the options of the WebSocket server blip connects to:

.. list-table::
   :header-rows: 1
   :widths: 25 20 55

   * - Option
     - Default
     - Description
   * - ``--ws-host``
     - ``127.0.0.1``
     - Address the WebSocket server binds to; ``0.0.0.0`` serves blips on
       other machines too, see :doc:`/help/limits`
   * - ``--ws-port``
     - ``3000``
     - WebSocket server port
   * - ``--ws-path``
     - ``/ws/outpost``
     - WebSocket endpoint path; connections to another path are rejected
       (the root path ``/`` is always accepted)
   * - ``--record``
     -
     - Record the messages sent to blip to this file
   * - ``--verbose`` / ``-v``
     - off
     - Debug output

tcp and udp
-----------

For targets that speak the blip protocol themselves, e.g. firmware using
:doc:`/sonde/index`::

   blip-outpost tcp HOST PORT [--listen] [options]
   blip-outpost udp HOST PORT [--listen] [options]

   blip-outpost tcp 192.168.1.70 9000
   blip-outpost udp 0.0.0.0 9000 --listen

.. list-table::
   :header-rows: 1
   :widths: 25 20 55

   * - Option
     - Default
     - Description
   * - ``--listen``
     - off
     - Accept connections (TCP) or datagrams (UDP) from the target at
       ``HOST PORT`` instead of connecting to it

* The target sends its messages as CBOR, any number at once as a CBOR
  sequence: each UDP datagram holds complete messages, over TCP they may be
  split anywhere. The outpost forwards each complete message to blip
  unchanged and drops data that is no CBOR. As a TCP client it reconnects
  every three seconds after an error.
* Invocations from blip are forwarded to the target as ``Invoke`` requests; an
  invocation counts as accepted once it is forwarded. The target announces its
  actions itself; ``ActionResult`` messages of the target are dropped, as the
  outpost answers blip itself.

protobuf
--------

For targets that send protobuf messages, e.g. over a WiFi bridge that forwards
the frames of a microcontroller::

   blip-outpost protobuf URL --proto FILE --uplink TYPE [options]

   blip-outpost protobuf ws://192.168.1.70/ws/robby \
       --proto link.proto -I proto --uplink my.link.Uplink --downlink my.link.Downlink

.. list-table::
   :header-rows: 1
   :widths: 25 20 55

   * - Option
     - Default
     - Description
   * - ``--proto``
     - required
     - The schema, a ``.proto`` file; may be given several times
   * - ``--include`` / ``-I``
     -
     - Directory to look for imports in (``protoc -I``); may be given several
       times
   * - ``--uplink``
     - required
     - Full name of the message type the target sends, e.g.
       ``robby.link.Uplink``
   * - ``--downlink``
     -
     - Full name of the message type the target receives; its actions are
       offered to blip
   * - ``--timeout``
     - ``1.0``
     - Seconds to wait for the answer of the target to an action
   * - ``--protoc``
     - ``protoc``
     - The ``protoc`` to compile the schema with
   * - ``--idle-timeout``
     - ``5.0``
     - Seconds without a message after which the target counts as lost and
       the outpost connects again; ``0`` waits forever

The outpost compiles the schema with ``protoc`` when it starts and builds the
message classes from it: no code is generated, so a changed schema takes effect
when the outpost starts again. Annotations in the schema say what each field is
to blip; a schema without them still works, every field then becomes a trace
named after it.

* The target is a WebSocket server at ``URL``. Each binary frame from it holds
  one serialized message of the ``--uplink`` type; each binary frame to it one
  of the ``--downlink`` type. A text frame from it holds JSON values of its own,
  ``{"values": {"bridge/rx_bytes": 1234}, "units": {"bridge/rx_bytes": "B"}}``,
  optionally with a ``"timestamp"`` in ns. The outpost reconnects every three
  seconds after an error.
* The actions are the members of a oneof in the downlink message, see below.
  An invocation sends a downlink message and waits ``--timeout`` seconds for
  the answer of the target, if the uplink message has one. The actions are
  disabled while the target is not connected.
* The outpost publishes its own counters once per second: ``outpost/frames``,
  ``outpost/decode_errors``, and with a sequence field ``outpost/seq_gaps`` and
  ``outpost/resets``.

The annotations are in ``blip/options.proto``, which comes with the outpost
(``src/blip_outpost/proto``) and is on the import path first; copy it next to
your schema if other tools compile the schema too. nanopb needs
``-x blip/options.proto`` so it does not include code for it.

.. code-block:: protobuf

   import "blip/options.proto";

   message Uplink {
     uint32 seq = 1 [(blip.field).role = ROLE_SEQUENCE];
     repeated Record records = 2;
   }

   message ImuBatch {
     option (blip.message) = {path: "imu/acc", record: true};
     uint64 t_first_ns = 1 [(blip.field).role = ROLE_TIMESTAMP];
     uint32 period_ns = 2 [(blip.field).role = ROLE_PERIOD];
     repeated sint32 x = 3 [(blip.field) = {unit: "g", scale: 0.000061}];
     repeated sint32 y = 4 [(blip.field) = {unit: "g", scale: 0.000061}];
   }

   message Task {
     option (blip.message) = {path: "tasks/{name}", record: true};
     uint64 t_ns = 1 [(blip.field).role = ROLE_TIMESTAMP];
     string name = 2;
     uint32 runtime_bp = 3 [(blip.field) = {name: "runtime", unit: "%", scale: 0.01}];
   }

The rules:

* **Paths.** The uplink message has the path ``""``. A message has the path of
  its ``(blip.message).path``, else the path of the field that holds it. A
  field adds its name, or its ``name`` (``""`` adds nothing); ``path`` gives a
  field a whole path of its own. ``{field}`` in a path is replaced by the value
  of that field of the message, which is then no value of its own.
* **Values.** Each scalar field is a trace: a single value, or a list for a
  repeated field (nested with ``shape``, e.g. ``[24, 8]``). A map of values
  is a record of its keys; in a map of messages each message is at the path
  of the map joined with its key. ``scale``
  multiplies the values (sent as floats), ``null_below`` sends values below it
  as ``null``, ``unit`` is shown in blip, ``skip`` leaves a field out. An enum
  is sent by its name: its ``(blip.value).name``, else the name of the value
  without the prefix of its enum, in lower case (``STATE_IDLE`` of ``State``
  becomes ``idle``). Fields that are not set (a oneof member or an
  ``optional`` field) are not sent.
* **Records.** With ``record: true`` the value fields of a message are one
  record value at its path, ``{name: value, ...}``.
* **Time.** The field with ``ROLE_TIMESTAMP`` (ns, times ``scale``) is the
  time of the values of its message and of the messages in it. Without one,
  values get the outpost's estimate of the time of the target: its newest
  timestamp plus the time since it arrived, or the time of the PC before the
  first.
* **Series.** With ``ROLE_PERIOD`` as well, the repeated fields are samples,
  sample ``i`` at ``timestamp + i × period``: one value per sample.
* **Sequence.** A field with ``ROLE_SEQUENCE`` in the uplink message counts
  frames: +1 per frame, wrapping; gaps and restarts (back to 0) are counted.
  In the downlink message the outpost sets it, +1 per message.
* **Log records.** A message of ``kind: KIND_LOG`` is a log record of its
  fields of the roles ``ROLE_LEVEL`` (0 trace … 4 error; larger values are
  errors), ``ROLE_COMPONENT`` and ``ROLE_MESSAGE``, at the path of the
  message like values: its ``path``, else that of the field holding it.
* **Actions.** The ``KIND_INVOKE`` message in the downlink message has a field
  of ``ROLE_ACTION_ID`` and a oneof: each message member is an action at its
  ``path`` (else its field name) with its ``description``, ``confirm`` and
  ``readback``, and the fields of the member are the parameters: integers,
  floats, bools, strings and enums, with ``min``, ``max``, ``step``, ``unit``
  and ``default`` (a parameter with a default may be left out). A message of
  ``kind: KIND_ACTION_RESULT`` in the uplink answers the invocation of its
  ``ROLE_ACTION_ID`` field, rejected with the text of its ``ROLE_ERROR`` field
  unless that is empty.

replay
------

Replays a recording, saved by blip or written with ``record`` or by sonde::

   blip-outpost replay FILE [--speed FACTOR] [options]

   blip-outpost replay recording.blip --speed 2

.. list-table::
   :header-rows: 1
   :widths: 25 20 55

   * - Option
     - Default
     - Description
   * - ``--speed``
     - ``1.0``
     - Replay speed; ``2`` replays twice as fast

The messages are sent unchanged, each when the time between its timestamp and
the first one has passed, divided by ``--speed``. The replay starts when the
first blip connects, and the outpost serves until stopped after it ended. The
actions of the recording are shown disabled, as a replay carries none out.
