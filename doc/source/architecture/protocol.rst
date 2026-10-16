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

blip to target: ``Action``
--------------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Variant
     - Payload
   * - ``SetMotorSpeed``
     - ``u16``

Text frames received by blip are logged but otherwise ignored.

.. note::

   On the WebSocket side each frame carries exactly one message.

Recording files
---------------

blip saves the received messages to a recording file (**File > Save
recording**), and the bridge writes the same format with ``--record``. A
recording is a CBOR sequence (RFC 8742), i.e. CBOR items one after another
without anything around them, compressed with gzip:

#. A header, the map ``{"format": "blip recording", "version": 1}``.
#. The ``Message`` of each binary frame, byte for byte as received, in the
   order of arrival.

As the frames are stored as received, a recording keeps everything that was
sent, also messages blip could not read. Appending a frame is writing its
bytes, so a recording that was cut off, e.g. when the bridge was killed, is
read up to the last whole message. Readers accept files of several gzip
members one after another, as appending to a gzip file creates them, and
uncompressed files.

blip saves recordings with the extension ``.blip``. Layouts are saved as JSON
files of the same versioned format that blip keeps its layout in, so they can
be read, compared and edited.
