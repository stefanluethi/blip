Wire protocol
=============

Messages are exchanged as **binary WebSocket frames** containing CBOR-encoded
values of the types defined in the ``proto`` crate (``proto/src/lib.rs``).
Enums use serde's externally tagged representation, i.e. a map with a single
key naming the variant.

Target to blip: ``Message``
---------------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Variant
     - Fields
   * - ``Hello``
     - Who is sending, see `Hello`_.
   * - ``Sample``
     - ``timestamp: u64`` (ns), ``path: String``, ``value: Data`` (see
       below). Called ``Value`` before protocol version 1; blip reads both
       names.
   * - ``Samples``
     - A block of samples of one trace, see `Blocks of samples`_.
   * - ``TraceMetadata``
     - What the target says of a trace, see `Trace metadata`_. Called
       ``ValueMetadata`` before, with a ``path`` and a ``unit`` only; blip reads
       both names.
   * - ``Log``
     - ``timestamp: u64`` (ns), ``path: String`` (optional, left out when
       empty), ``level: u8``, ``message: String``. A ``component`` key of an
       older sender is ignored. See `Log records`_.
   * - ``Actions``
     - ``Vec<ActionDescriptor>``, all actions the target can carry out (see
       `Actions`_)
   * - ``ActionResult``
     - ``id: u32``, ``result: Result<(), String>``

The outpost forwards the old names ``Value`` and ``ValueMetadata`` of a target
it passes through unchanged and writes the new ones itself.

Example (shown as JSON for readability)::

   {"Sample": {"timestamp": 1712345678000000000, "path": "cpu_load", "value": 12.5}}

Hello
~~~~~

A target sends a ``Hello`` first, once for each connection. A second one
replaces the first. All of it but the version is optional, left out when
empty.

.. list-table:: ``Hello``
   :header-rows: 1
   :widths: 20 80

   * - Field
     - Meaning
   * - ``version: u32``
     - The version of the protocol the sender speaks: 1 (``proto::VERSION``).
       blip reads up to 1. For a newer version it warns and reads on, showing
       what it knows, see :doc:`/blip/connecting`.
   * - ``name: String``
     - Optional, the name of the target, e.g. the host name of the outpost.
   * - ``clock: String``
     - Optional, what the timestamps count from: ``"epoch"`` for the Unix
       epoch (UTC) or ``"boot"`` for the time since the target booted. A clock
       blip does not know, e.g. of a later protocol, counts as none.
   * - ``sender: String``
     - Optional, the software and version that sends, e.g. ``"sonde 1.2"``.

Without a ``Hello``, or with one without a ``clock``, blip guesses the time
base, see `Time base`_. Example::

   {"Hello": {"version": 1, "name": "rig-7", "clock": "boot", "sender": "sonde 0.2.0"}}

Trace metadata
~~~~~~~~~~~~~~

A ``TraceMetadata`` says what the values of a trace mean. Fields left out are
not set; empty ones are not written.

.. list-table:: ``TraceMetadata``
   :header-rows: 1
   :widths: 20 80

   * - Field
     - Meaning
   * - ``path: String``
     - The trace the metadata is of.
   * - ``unit: String``
     - Optional, e.g. ``"rpm"``. An empty unit is no unit.
   * - ``description: String``
     - Optional, what the trace is, in a sentence.
   * - ``enum``
     - Optional, labels for integer values: a CBOR map from integer to text,
       e.g. ``{0: "idle", 1: "run", 2: "fault"}``. The key is an ``i64``.
   * - ``range``
     - Optional, the range the values are expected in: a map ``{"min": …,
       "max": …}`` of numbers (``f64``, integers are accepted), ``min`` below
       ``max``. A range that is not finite or does not have ``min`` below
       ``max`` is dropped, with a warning, and the rest kept.

A newer ``TraceMetadata`` of a path replaces all earlier metadata of that path,
so send everything about a trace in each. It applies to the trace at its path,
also a field trace of a record (``imu.acc``), and to the element traces of an
array trace (``imu.acc[0]``). Example::

   {"TraceMetadata": {"path": "motor/mode", "description": "Operating mode",
                      "enum": {0: "Idle", 1: "Ramp", 2: "Run", 3: "Fault"}}}
   {"TraceMetadata": {"path": "motor.speed_rpm", "unit": "rpm",
                      "range": {"min": 0, "max": 3000}}}

Blocks of samples
~~~~~~~~~~~~~~~~~

A ``Samples`` message carries many samples of one trace, without repeating the
path and the timestamp of each. It has the ``path``, a list ``values`` of ``Data``
and the times in one of two forms:

* ``timestamp`` and ``period``, both ``u64`` in ns: sample ``i`` is at
  ``timestamp + i * period``.
* ``timestamps``, a list of ``u64`` in ns of the same length as ``values``.

Exactly one of ``period`` and ``timestamps`` is present, and a ``period`` goes
with a ``timestamp``. A block may have no values. blip gives the same traces
as for one ``Sample`` per value. It checks a block whole: one that is not
well formed, e.g. with neither form of times, with both, with lists of different
lengths, or with times beyond 2\ :sup:`64` ns, is dropped alone with an error
notice, and the other messages of its frame are kept. Example::

   {"Samples": {"path": "adc", "values": [512, 514, 513],
                "timestamp": 1712345678000000000, "period": 1000000}}
   {"Samples": {"path": "vibration", "values": [0.1, 0.4],
                "timestamps": [1712345678000000000, 1712345678000250000]}}

Values: ``Data``
~~~~~~~~~~~~~~~~

A ``value`` is a plain CBOR data item without any type tag, so a value costs no
more than its CBOR encoding, also in stored recordings. The CBOR major type
decides the kind of the value:

.. list-table::
   :header-rows: 1
   :widths: 20 30 50

   * - Kind
     - CBOR
     - Notes
   * - integer
     - unsigned / negative integer
     - One kind on the wire: the sign alone decides the major type. Values
       range from ``i64::MIN`` to ``u64::MAX``.
   * - float
     - float (16, 32 or 64 bit)
     - Decoded as ``f64``. Encoders should use the shortest width that keeps
       the value, as the ``proto`` crate does.
   * - bool
     - ``true`` / ``false``
     -
   * - string
     - text string
     -
   * - array
     - array
     - Elements of any kind. An array keeps the same shape in every sample of
       a trace.
   * - record
     - map with text string keys
     - Fields of any kind, field names are unique.
   * - no value
     - ``null``
     - A sample without a value, e.g. a failed measurement. Plots show a gap.

Byte strings, maps with other keys and other simple values are rejected.

The kind of a trace is set by its first value other than ``null``. Integer
and float are compatible, a trace switches to float when its first float
arrives. A sample of any other kind is dropped with a warning.

Example of a record with an array field::

   {"Sample": {"timestamp": 1712345678000000000, "path": "imu",
                "value": {"acc": [0.1, -0.2, 9.8], "valid": true}}}

blip to target: ``Request``
---------------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Variant
     - Fields
   * - ``Invoke``
     - ``id: u32``, ``path: String``, ``args: Data`` (a record)

Text frames received by blip are logged but otherwise ignored.

.. note::

   A WebSocket frame to blip carries one message or several as a CBOR
   sequence (RFC 8742), i.e. their CBOR items one after another, e.g. a batch
   a target sent in one datagram. blip applies them in their order; if the
   frame cannot be read to its end, the messages before are kept. A frame to
   the target carries exactly one request.

   The outpost sends the messages queued for a blip together, up to 64 KiB
   in a frame, a larger message in a frame of its own. A client that reads
   one message per frame connects with ``?batch=0`` after the path, e.g.
   ``ws://127.0.0.1:3000/ws/outpost?batch=0``. Targets behind its TCP/UDP
   passthrough may send CBOR sequences: each UDP datagram holds complete
   messages, while over TCP the outpost reassembles messages split across
   reads.

Log records
-----------

The ``level`` of a ``Log`` is 0 Trace, 1 Debug, 2 Info, 3 Warn or 4 Error
(``proto::level``). blip shows a level above 4 as Error. A record without a
``path`` is at ``log/default``. A ``timestamp`` of 0 means *now*: blip puts the
record at the newest time of the data received so far, so it lines up with the
samples of its target, or at the time of day of the computer if there is no
data yet.

Time base
---------

All timestamps are in nanoseconds. What they count from, the **time base**, is
the ``clock`` of the ``Hello`` of the connection: the Unix epoch (UTC, shown as
the time of day) or the time since boot. Without a clock blip guesses: a
timestamp before 10\ :sup:`18` ns (2001-09-09) is the time since boot, any
other the Unix epoch.

blip shows all data on one time axis, so one time base. The first sample or
log record with a timestamp, or an epoch ``Hello``, sets it, and samples of the
other base are dropped, with one warning. If connections declare different
clocks, blip warns once and the epoch wins: it removes the samples and log
records of the time since boot it already has, and drops the later ones. The
metadata, the actions and the recording are kept.

Actions
-------

The target announces the actions it can carry out, e.g. setting a motor
setpoint, a log level or enabling a function, and blip invokes them.

An ``Actions`` message lists all actions and replaces the list sent before, so
actions that are gone disappear in blip. The outpost sends the list to each
client that connects and to all clients whenever it changes.

.. list-table:: ``ActionDescriptor``
   :header-rows: 1
   :widths: 25 75

   * - Field
     - Meaning
   * - ``path: String``
     - Names the action, levels separated by ``/`` like trace paths.
   * - ``description: String``
     - Optional.
   * - ``params: Vec<Param>``
     - The parameters, optional. An action without parameters is a button.
   * - ``readback: Option<String>``
     - Optional, the path of the trace that shows the current value of what
       the action sets, e.g. the measured speed of a speed setpoint. It is
       metadata only: blip shows the value next to the action and starts the
       input from it.
   * - ``confirm: bool``
     - Optional, whether the user confirms before the action is sent.
   * - ``enabled: bool``
     - Optional (default ``true``), whether the target can carry out the
       action now, e.g. not while the link to the device is down.
   * - ``disabled_reason: Option<String>``
     - Optional, why the target cannot carry out the action now, which blip
       shows while it is not ``enabled``, e.g. ``"calibrating"``.

A ``Param`` has a ``name: String``, a ``kind: ParamKind``, a ``unit: String``
(optional) and a ``default: Option<Data>`` (optional). The kinds:

.. list-table:: ``ParamKind``
   :header-rows: 1
   :widths: 40 60

   * - Kind (CBOR)
     - Argument
   * - ``"Bool"``
     - bool
   * - ``{"Int": {"min": …, "max": …}}``
     - integer, bounds optional
   * - ``{"Float": {"min": …, "max": …, "step": …}}``
     - float (or integer), bounds and step optional
   * - ``{"Enum": ["a", "b"]}``
     - one of the strings
   * - ``"String"``
     - string

.. figure:: /diagrams/action-sequence.svg
   :target: ../_images/action-sequence.svg
   :class: diagram
   :alt: Sequence of announcing, invoking, answering and reading back an
         action.

   The messages of an invocation.

An ``Invoke`` request carries an ``id`` chosen by blip, unique within the
connection, the ``path`` of the action and its arguments as a record by
parameter name. The target answers the client that sent it with an
``ActionResult`` of the same ``id``, ``{"Ok": null}`` if it carried the action
out, ``{"Err": "reason"}`` if it rejected it. Whether the action had the
desired effect shows in the traces, e.g. the readback. The outpost answers the
invocations itself and drops ``ActionResult`` messages sent to it, e.g.
those of a passthrough target or of a replayed recording.

Example (shown as JSON for readability)::

   {"Actions": [{"path": "motor/speed", "readback": "motor/speed_rpm",
                 "params": [{"name": "rpm", "unit": "rpm",
                             "kind": {"Int": {"min": 0, "max": 3000}}}]}]}
   {"Invoke": {"id": 1, "path": "motor/speed", "args": {"rpm": 1500}}}
   {"ActionResult": {"id": 1, "result": {"Ok": null}}}

.. _recording-files:

Recording files
---------------

blip saves the received messages to a recording file (**File › Save
recording**), the outpost writes the same format when asked to record and sonde
with its ``FileSink``. A recording is a CBOR sequence (RFC 8742), i.e. CBOR items one after another
without anything around them, compressed with gzip:

.. figure:: /diagrams/recording-file.svg
   :target: ../_images/recording-file.svg
   :class: diagram
   :alt: A recording file: a header, then messages, with text strings naming
         the connection of the messages after them, all gzip-compressed.

   Layout of a recording file.

#. A header, the map ``{"format": "blip recording", "version": 3}``.
#. The ``Message`` of each binary frame, byte for byte as received, in the
   order of arrival.
#. Since version 2, a text string before the messages of another connection
   with the name of that connection, see :doc:`/blip/connecting`. Messages are maps, so
   a name cannot be taken for a message. Messages before the first name, and
   all messages of version 1, have no connection name.

.. list-table:: Versions of the format
   :header-rows: 1
   :widths: 10 90

   * - Version
     - Change
   * - 1
     - The first: the header and the messages, no connection names.
   * - 2
     - Adds the connection names between the messages.
   * - 3
     - Written since the messages ``Hello``, ``Sample``, ``Samples`` and
       ``TraceMetadata`` exist; the layout of the file is that of version 2.
       The messages of earlier versions keep their old names ``Value`` and
       ``ValueMetadata``, and have no ``Hello``, so blip guesses their time
       base.

Each writer keeps to a layout:

* **blip** writes version 3 in the layout of version 2, with connection
  names, gzip-compressed.
* The **outpost** writes version 3 in the layout of version 1, without names,
  gzip-compressed.
* **sonde** (``FileSink``) writes version 3 in the layout of version 1,
  without names and uncompressed. Its file starts with the ``Hello`` of its
  queue.

blip and the outpost read versions 1 to 3 and refuse a newer one; the outpost
replays the messages of all connections of a file without their names.

As the frames are stored as received, a recording keeps everything that was
sent, also messages blip could not read. Appending a frame is writing its
bytes, so a recording that was cut off, e.g. when the outpost was killed, is
read up to the last whole message. Readers accept files of several gzip
members one after another, as appending to a gzip file creates them, and
uncompressed files.

blip saves recordings with the extension ``.blip``. Layouts are saved as JSON
files of the same versioned format that blip keeps its layout in, so they can
be read, compared and edited.
