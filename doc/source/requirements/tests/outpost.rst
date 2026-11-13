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

.. test:: Simulator outpost declares and blocks
   :id: TEST_OUTPOST_SIM_META
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM

   ``outpost/tests/test_simulator.py::test_simulator_outpost_declares_and_blocks``
   -- connects a WebSocket client to the outpost in simulator mode and
   checks that it gets a ``Hello`` with the hostname and ``epoch`` clock, a
   ``TraceMetadata`` with a description, one with labels and one with a range,
   and at least one ``Samples`` block that is well-formed.

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

.. test:: The outpost sends and keeps a Hello
   :id: TEST_OUTPOST_HELLO
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_HELLO

   ``outpost/tests/test_outpost.py::test_a_client_gets_a_hello_first``,
   ``test_name_and_clock_are_settable``, ``test_the_module_options_set_name_and_clock``,
   ``test_a_hello_the_script_sends_replaces_its_own`` and
   ``test_the_recording_starts_with_the_units_and_actions_sent_before`` check that a
   client connecting to a script gets a ``Hello`` first with the name, clock (by
   default the hostname and ``epoch``, both settable, a wrong clock refused) and the
   sender ``blip-outpost`` with its version, that a ``Hello`` the script sends
   replaces its own, and that the recording has it.
   ``outpost/tests/test_socket.py::test_the_latest_hello_of_a_target_reaches_blips_connecting_later_and_a_new_one_is_forwarded``
   checks that the latest ``Hello`` of a passthrough target is given to clients
   connecting later and a new one is forwarded;
   ``test_the_passthrough_the_replay_and_the_protobuf_link_send_no_hello_of_their_own``
   (and ``outpost/tests/test_protobuf.py::test_the_protobuf_link_sends_no_hello_of_its_own``)
   that they send none of their own;
   ``outpost/tests/test_server.py::test_a_client_gets_the_hello_before_the_cached_messages``
   the order and the cache of the server;
   ``outpost/tests/test_socket.py::test_old_names_of_a_target_are_forwarded_unchanged_and_the_metadata_is_kept``
   that ``Value`` and ``ValueMetadata`` of a passthrough target are forwarded unchanged.

.. test:: The outpost sends metadata
   :id: TEST_OUTPOST_METADATA
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_METADATA

   ``outpost/tests/test_metadata.py`` -- checks that the unit, description, labels and range of a trace,
   and those of the fields of a dataclass, are sent as ``TraceMetadata`` once
   per path, that a client connecting later gets them, and that ones without
   the optional fields are as before.

.. test:: The outpost sends blocks of samples
   :id: TEST_OUTPOST_SAMPLES
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SAMPLES

   ``outpost/tests/test_samples.py::test_a_block_has_a_period_or_timestamps``,
   ``outpost/tests/test_samples.py::test_a_malformed_block_is_refused``,
   ``outpost/tests/test_samples.py::test_a_script_hands_a_block_over_as_one_message``,
   ``outpost/tests/test_samples.py::test_a_batch_hands_a_block_over_with_its_messages``,
   ``outpost/tests/test_tracer.py::test_a_series_is_sent_at_its_timestamps`` --
   checks that a series with timestamps and one with a period are
   sent as one ``Samples`` message each, with exactly one of them and values of
   the same length.
