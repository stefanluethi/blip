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
   current sample of the x trace, and sample arrays over time for heatmaps.

.. test:: Time cursor is set and dragged
   :id: TEST_BLIP_CURSOR
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_CURSOR

   ``blip/src/app/time.rs`` and ``blip/src/app/view/time_axis.rs`` tests --
   keep the cursor at the end of the range until set, stop live mode when it
   is set, and click and drag it in a plot without panning the plot.

.. test:: Trace values encode as plain CBOR
   :id: TEST_PROTO_DATA
   :status: implemented
   :component: proto
   :verifies: REQ_PROTO_TYPES, REQ_PROTO_CBOR

   ``proto/src/data.rs`` unit tests -- encode every kind of ``Data`` to the
   expected CBOR bytes, round-trip nested arrays and records, decode values
   encoded by the Python bridge, and reject byte strings, non-text map keys,
   duplicate fields and integers out of range.

.. test:: Integers are kept exactly
   :id: TEST_BLIP_NUMBERS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_INT

   ``blip/src/app/model/numbers.rs`` and ``blip/src/app/model/traces.rs``
   tests -- keep integers beyond 2^53 exact, switch to unsigned for integers
   above ``i64::MAX``, reject integers that do not fit with the others, and
   write them in every number format.

.. test:: Recordings are saved and opened
   :id: TEST_BLIP_RECORDING
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_RECORDING

   ``blip/src/app/model/recording.rs`` tests -- save frames and read them
   back unchanged, read uncompressed files, files of several gzip members and
   files written by the Python bridge, read a cut off file up to the cut and
   reject other files.

.. test:: Layouts are saved as JSON
   :id: TEST_BLIP_LAYOUT_FILE
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_LAYOUT

   ``blip/src/app/layout/file.rs::layouts_survive_json`` -- saves a layout
   with a view of each type as JSON and checks it loads unchanged.

.. test:: Bridge records and replays
   :id: TEST_BRIDGE_RECORDING
   :status: implemented
   :component: bridge
   :verifies: REQ_BRIDGE_RECORD, REQ_BRIDGE_REPLAY

   ``bridge/tests/test_recording.py`` -- writes and reads recording files,
   also cut off and uncompressed ones, and replays a recording through the
   bridge to a WebSocket client while recording what is sent, checking that
   both are the recorded frames byte for byte.
