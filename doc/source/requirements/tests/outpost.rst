Outpost
=======

Tests of the requirements in :doc:`/requirements/system/outpost`.

.. test:: Simulator outpost streams values
   :id: TEST_OUTPOST_SIM
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM, REQ_PROTO_CBOR

   ``outpost/tests/test_simulator.py::test_simulator_outpost`` -- connects a
   WebSocket client to the outpost in simulator mode and checks that valid
   CBOR messages are received.

.. test:: Simulator outpost streams logs
   :id: TEST_OUTPOST_SIM_LOGS
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM, REQ_PROTO_CBOR

   ``outpost/tests/test_simulator.py::test_simulator_outpost_logs`` -- connects a
   WebSocket client to the outpost in simulator mode and checks that well-formed
   ``Log`` records are received.

.. test:: Simulator outpost streams typed values
   :id: TEST_OUTPOST_SIM_TYPES
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM, REQ_PROTO_TYPES

   ``outpost/tests/test_simulator.py::test_simulator_outpost_typed_values`` --
   connects a WebSocket client to the outpost in simulator mode and checks that
   records, arrays, integers, bools, strings and nulls arrive as plain CBOR
   items.

.. test:: Simulator outpost streams traces at different rates
   :id: TEST_OUTPOST_SIM_RATES
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM

   ``outpost/tests/test_simulator.py::test_simulator_outpost_rates`` -- connects
   a WebSocket client to the outpost in simulator mode and checks that faster
   sampled traces arrive more often than slower ones.

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

.. test:: Outpost serves a script
   :id: TEST_OUTPOST_LIBRARY
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_PROTOCOL, REQ_OUTPOST_RECORD

   ``outpost/tests/test_outpost.py`` -- serves from a thread of its own and
   passes on what another thread sends, in order; reports a port in use;
   sends the units and actions sent before to blips connecting later and to
   the recording; announces actions changed from another thread; finishes the
   recording on stop; and makes functions actions right away and methods those
   of registered objects.

.. test:: Tracers, batches and records
   :id: TEST_OUTPOST_TRACERS
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_BATCH, REQ_OUTPOST_TRACERS

   ``outpost/tests/test_tracer.py`` -- the same name gives the same tracer, its
   traces, actions and readbacks are below its name and its log records at
   its name, formatted as ``logging`` does; forwarded log records carry
   their own path, and ``log`` and the tracer take no component; a dataclass is a record
   whose units are sent once per path, also of the fields whose annotations
   resolve when others do not; numpy-like values are sent as lists; a series
   is sent at its timestamps or its period; a batch is handed over at its end,
   also when it raises, in one handover from another thread, and with one
   timestamp, shared by the batches nested in it; a single mapping formats by
   name; ``LogHandler`` maps the levels of ``logging`` and
   skips the outpost's own records. ``outpost/tests/test_app.py`` -- the
   example ``app.py`` traces its parts below their names.

.. test:: Simulator actions change what it sends
   :id: TEST_OUTPOST_SIM_ACTIONS
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM

   ``outpost/tests/test_simulator.py::test_simulator_actions_change_what_it_sends``
   -- the motor follows its setpoint and rejects it while disabled, the log
   level filters log records and is sent as its readback, and the
   calibration is disabled while it runs.

.. test:: Python logging is sent at paths
   :id: TEST_OUTPOST_LOGGING
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_LOGGING

   ``outpost/tests/test_tracer.py::test_the_log_handler_sends_loggers_below_log_by_default``
   -- ``LogHandler`` sends the logger ``app.motor`` at ``log/app/motor`` with
   the default prefix and at ``robot/app/motor`` below a prefix that is set;
   ``test_the_root_logger_is_at_the_prefix_or_the_target_as_a_whole`` -- the
   root logger is at an empty path, or at the prefix when one is set;
   ``test_log_records_have_no_component`` -- an encoded record has no
   ``component``; and
   ``outpost/tests/test_protobuf.py::test_log_records_take_level_text_and_the_component_as_a_path_level``
   -- the protobuf mapping appends the value of its component field to the
   path as one more level, below ``log`` when the path is empty.
