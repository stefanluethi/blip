Wire Protocol
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
   * - ``Value``
     - ``timestamp: u64`` (ns), ``path: String``, ``value: Data`` (see
       below)
   * - ``ValueMetadata``
     - ``path: String``, ``unit: String``
   * - ``Log``
     - ``timestamp: u64`` (ns), ``level: u8``, ``component: String``
       (optional), ``message: String``
   * - ``Actions``
     - ``Vec<ActionDescriptor>``, all actions the target can carry out (see
       `Actions`_)
   * - ``ActionResult``
     - ``id: u32``, ``result: Result<(), String>``

Example (shown as JSON for readability)::

   {"Value": {"timestamp": 1712345678000000000, "path": "cpu_load", "value": 12.5}}

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

   {"Value": {"timestamp": 1712345678000000000, "path": "imu",
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
invocations itself and drops ``ActionResult`` messages a protocol sends, e.g.
those of a passthrough target or of a replayed recording.

Example (shown as JSON for readability)::

   {"Actions": [{"path": "motor/speed", "readback": "motor/speed_rpm",
                 "params": [{"name": "rpm", "unit": "rpm",
                             "kind": {"Int": {"min": 0, "max": 3000}}}]}]}
   {"Invoke": {"id": 1, "path": "motor/speed", "args": {"rpm": 1500}}}
   {"ActionResult": {"id": 1, "result": {"Ok": null}}}

Recording files
---------------

blip saves the received messages to a recording file (**File › Save
recording**), the outpost writes the same format with ``--record`` and sonde
with its ``FileSink``. A recording is a CBOR sequence (RFC 8742), i.e. CBOR items one after another
without anything around them, compressed with gzip:

.. figure:: /diagrams/recording-file.svg
   :target: ../_images/recording-file.svg
   :class: diagram
   :alt: A recording file: a header, then messages, with text strings naming
         the connection of the messages after them, all gzip-compressed.

   Layout of a recording file.

#. A header, the map ``{"format": "blip recording", "version": 2}``.
#. The ``Message`` of each binary frame, byte for byte as received, in the
   order of arrival.
#. Since version 2, a text string before the messages of another connection
   with the name of that connection, see :doc:`/usage/blip/connecting`. Messages are maps, so
   a name cannot be taken for a message. Messages before the first name, and
   all messages of version 1, have no connection name. The outpost and sonde
   write version 1 (sonde uncompressed); the outpost reads version 2 and
   replays the messages of all its connections without their names.

As the frames are stored as received, a recording keeps everything that was
sent, also messages blip could not read. Appending a frame is writing its
bytes, so a recording that was cut off, e.g. when the outpost was killed, is
read up to the last whole message. Readers accept files of several gzip
members one after another, as appending to a gzip file creates them, and
uncompressed files.

blip saves recordings with the extension ``.blip``. Layouts are saved as JSON
files of the same versioned format that blip keeps its layout in, so they can
be read, compared and edited.
