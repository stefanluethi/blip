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
more than its CBOR encoding, also in stored traces. The CBOR major type decides
the kind of the value:

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
       a stream.
   * - record
     - map with text string keys
     - Fields of any kind, field names are unique.
   * - no value
     - ``null``
     - A sample without a value, e.g. a failed measurement. Plots show a gap.

Byte strings, maps with other keys and other simple values are rejected.

The kind of a stream is set by its first value other than ``null``. Integer
and float are compatible, a stream switches to float when its first float
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
