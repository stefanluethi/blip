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
