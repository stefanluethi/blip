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
     - ``timestamp: u64`` (ns), ``path: String``, ``value: f32``
   * - ``ValueMetadata``
     - ``path: String``, ``unit: String``
   * - ``Log``
     - ``timestamp: u64`` (ns), ``level: u8``, ``component: String``
       (optional), ``message: String``

Example (shown as JSON for readability)::

   {"Value": {"timestamp": 1712345678000000000, "path": "cpu_load", "value": 12.5}}

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

   On serial links the bridge frames CBOR payloads using COBS. On the
   WebSocket side each frame carries exactly one message.
