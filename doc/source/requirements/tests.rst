Test Cases
==========

.. test:: Simulator outpost streams values
   :id: TEST_OUTPOST_SIM
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM, REQ_PROTO_CBOR

   ``outpost/tests/test_outpost.py::test_simulator_outpost`` -- connects a
   WebSocket client to the outpost in simulator mode and checks that valid
   CBOR messages are received.

.. test:: Simulator outpost streams logs
   :id: TEST_OUTPOST_SIM_LOGS
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM, REQ_PROTO_CBOR

   ``outpost/tests/test_outpost.py::test_simulator_outpost_logs`` -- connects a
   WebSocket client to the outpost in simulator mode and checks that well-formed
   ``Log`` records are received.

.. test:: Simulator outpost streams typed values
   :id: TEST_OUTPOST_SIM_TYPES
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM, REQ_PROTO_TYPES

   ``outpost/tests/test_outpost.py::test_simulator_outpost_typed_values`` --
   connects a WebSocket client to the outpost in simulator mode and checks that
   records, arrays, integers, bools, strings and nulls arrive as plain CBOR
   items.

.. test:: Simulator outpost streams traces at different rates
   :id: TEST_OUTPOST_SIM_RATES
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM

   ``outpost/tests/test_outpost.py::test_simulator_outpost_rates`` -- connects
   a WebSocket client to the outpost in simulator mode and checks that faster
   sampled traces arrive more often than slower ones.

.. test:: Views compute what they show
   :id: TEST_BLIP_VIEWS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_STATES, REQ_VIS_TABLE, REQ_VIS_XY, REQ_VIS_HEATMAP

   ``blip/src/app/view`` unit tests -- merge equal values into state runs
   with gaps at nulls, hide collapsed rows of the table, pair samples with the
   current sample of the x trace, sample arrays over time for heatmaps, and
   zoom plots with the mouse wheel without panning them, unless disabled.

.. test:: Time cursor is set and dragged
   :id: TEST_BLIP_CURSOR
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_CURSOR

   ``blip/src/app/time.rs`` and ``blip/src/app/view/time_axis.rs`` tests --
   keep the cursor at the end of the range until set, stop live mode when it
   is set, and click and drag it in a plot without panning the plot.

.. test:: Recording is switched, the time range paused and played
   :id: TEST_BLIP_PLAYBACK
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_PLAYBACK

   ``blip/src/app.rs``, ``blip/src/app/time.rs``,
   ``blip/src/app/view/time_axis.rs`` and ``blip/src/app/model.rs`` tests --
   drop values and log records while not recording but keep units,
   actions and answers, start a recording only while connected, clearing the
   data but the latest units and actions, which are recorded again, stop
   recording when a recording file is opened, keep a paused range while data
   arrives, play on from it with the cursor without skipping ahead by the
   paused time, return to live mode at the newest data, pause when panning or
   setting the cursor but not when zooming, also with the mouse wheel over a
   plot, stay unchanged when zooming beyond the shortest or longest window, and
   switch with the buttons of the bottom panel.

.. test:: Trace values encode as plain CBOR
   :id: TEST_PROTO_DATA
   :status: implemented
   :component: proto
   :verifies: REQ_PROTO_TYPES, REQ_PROTO_CBOR

   ``proto/src/data.rs`` unit tests -- encode every kind of ``Data`` to the
   expected CBOR bytes, round-trip nested arrays and records, decode values
   encoded by the Python outpost, and reject byte strings, non-text map keys,
   duplicate fields and integers out of range.

.. test:: Frames are split into their messages
   :id: TEST_BLIP_CBOR_SEQUENCE
   :status: implemented
   :component: blip
   :verifies: REQ_PROTO_SEQUENCE

   ``blip/src/app/model.rs::a_cbor_sequence_is_split_into_its_items`` --
   splits a CBOR sequence of a value and a log record into both messages, and
   one cut off or broken in the middle into the messages before and an error.

.. test:: Integers are kept exactly
   :id: TEST_BLIP_NUMBERS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_INT

   ``blip/src/app/model/numbers.rs`` and ``blip/src/app/model/traces.rs``
   tests -- keep integers beyond 2^53 exact, switch to unsigned for integers
   above ``i64::MAX``, reject integers that do not fit with the others, and
   write them in every number format.

.. test:: Connections are edited, opened and their errors notified
   :id: TEST_BLIP_CONNECTIONS
   :status: implemented
   :component: blip
   :verifies: REQ_CONN_WS, REQ_CONN_ERRORS

   ``blip/src/app/connections.rs``, ``blip/src/app/toasts.rs`` and
   ``blip/src/app.rs`` tests -- name connections after their address, add
   them with the + button, open several at once, keep them from being changed
   while recording, report why an invalid
   address fails to connect, show errors and warnings as toasts until they
   expire or are closed, count a repeated one instead of stacking it, and
   warn when a recording is opened in part.

.. test:: Recordings are saved and opened
   :id: TEST_BLIP_RECORDING
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_RECORDING

   ``blip/src/app/model/recording.rs`` tests -- save frames and read them
   back unchanged, read uncompressed files, files of several gzip members and
   files written by the Python outpost, read a cut off file up to the cut and
   reject other files.

.. test:: Layouts are saved as JSON
   :id: TEST_BLIP_LAYOUT_FILE
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_LAYOUT

   ``blip/src/app/layout/file.rs::layouts_survive_json`` -- saves a layout
   with a view of each type as JSON and checks it loads unchanged.

.. test:: Outpost records and replays
   :id: TEST_OUTPOST_RECORDING
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_RECORD, REQ_OUTPOST_REPLAY

   ``outpost/tests/test_recording.py`` -- writes and reads recording files,
   also cut off and uncompressed ones, and replays a recording through the
   outpost to a WebSocket client while recording what is sent, checking that
   both are the recorded frames byte for byte.

.. test:: Outpost splits the batches of socket targets
   :id: TEST_OUTPOST_SOCKET
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SOCKET

   ``outpost/tests/test_socket.py`` -- forwards several messages of one TCP
   read and of one UDP datagram one by one, a message split over reads once
   it is complete, keeps an ``Actions`` and drops an ``ActionResult`` within a
   batch, drops undecodable data and messages that do not end, and keeps
   reconnecting after errors.

.. test:: Action messages encode as plain CBOR
   :id: TEST_PROTO_ACTIONS
   :status: implemented
   :component: proto
   :verifies: REQ_CTRL_ANNOUNCE, REQ_CTRL_FEEDBACK

   ``proto/src/action.rs`` unit tests -- encode invocations and results as
   the documented CBOR maps, parameter kinds as strings or maps, and decode
   announcements whose optional fields are left out.

.. test:: Outpost announces, checks and carries out actions
   :id: TEST_OUTPOST_ACTIONS
   :status: implemented
   :component: outpost
   :verifies: REQ_CTRL_ANNOUNCE, REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK, REQ_OUTPOST_PROTOCOL

   ``outpost/tests/test_actions.py`` -- takes the parameters of actions from
   their type hints and rejects unsupported ones, checks and converts
   arguments, rejects disabled actions and those that raise, sends the
   actions to connecting clients and changed ones to all, answers only the
   invoking client, logs to all, and runs a slow action without holding up
   others.

.. test:: Outpost loads user-provided protocols
   :id: TEST_OUTPOST_LOADING
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_PROTOCOL

   ``outpost/tests/test_loading.py`` -- loads built-in protocols by name and
   user protocols from files with and without a class name, converts the
   options by the type hints of the constructor and reports unknown, missing
   and invalid ones.

.. test:: Simulator actions change what it sends
   :id: TEST_OUTPOST_SIM_ACTIONS
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM

   ``outpost/tests/test_outpost.py::test_simulator_actions_change_what_it_sends``
   -- the motor follows its setpoint and rejects it while disabled, the log
   level filters log records and is sent as its readback, and the
   calibration is disabled while it runs.

.. test:: blip invokes actions and shows their outcome
   :id: TEST_BLIP_ACTIONS
   :status: implemented
   :component: blip
   :verifies: REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK, REQ_CTRL_VIEW, REQ_CTRL_SAFE

   ``blip/src/app/model/actions.rs`` and ``blip/src/app/view/actions.rs``
   tests -- send invocations with new ids, show only the outcome of the latest
   invocation of an action, time out unanswered ones, send at most every
   interval while dragging and the final value on release; send buttons and
   checkboxes right away and actions with several parameters with their Send
   button, ask before confirmed actions, disable actions while disconnected
   or disabled, and start inputs from the readback or the default.
