Test Cases
==========

.. test:: Simulator bridge streams values
   :id: TEST_BRIDGE_SIM
   :status: implemented
   :component: bridge
   :verifies: REQ_BRIDGE_SIM, REQ_PROTO_CBOR

   ``bridge/tests/test_bridge.py::test_simulator_bridge`` -- connects a
   WebSocket client to the bridge in simulator mode and checks that valid
   CBOR messages are received.

.. test:: Simulator bridge streams logs
   :id: TEST_BRIDGE_SIM_LOGS
   :status: implemented
   :component: bridge
   :verifies: REQ_BRIDGE_SIM, REQ_PROTO_CBOR

   ``bridge/tests/test_bridge.py::test_simulator_bridge_logs`` -- connects a
   WebSocket client to the bridge in simulator mode and checks that well-formed
   ``Log`` records are received.

.. test:: Simulator bridge streams typed values
   :id: TEST_BRIDGE_SIM_TYPES
   :status: implemented
   :component: bridge
   :verifies: REQ_BRIDGE_SIM, REQ_PROTO_TYPES

   ``bridge/tests/test_bridge.py::test_simulator_bridge_typed_values`` --
   connects a WebSocket client to the bridge in simulator mode and checks that
   records, arrays, integers, bools, strings and nulls arrive as plain CBOR
   items.

.. test:: Views compute what they show
   :id: TEST_BLIP_VIEWS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_STATES, REQ_VIS_TABLE, REQ_VIS_XY, REQ_VIS_HEATMAP

   ``blip/src/app/view`` unit tests -- merge equal values into state runs
   with gaps at nulls, hide collapsed rows of the table, pair samples with the
   current sample of the x stream, and sample arrays over time for heatmaps.

.. test:: Stream values encode as plain CBOR
   :id: TEST_PROTO_DATA
   :status: implemented
   :component: proto
   :verifies: REQ_PROTO_TYPES, REQ_PROTO_CBOR

   ``proto/src/data.rs`` unit tests -- encode every kind of ``Data`` to the
   expected CBOR bytes, round-trip nested arrays and records, decode values
   encoded by the Python bridge, and reject byte strings, non-text map keys,
   duplicate fields and integers out of range.
