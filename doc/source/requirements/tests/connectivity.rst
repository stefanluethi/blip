Connectivity
============

Tests of the requirements in :doc:`/requirements/system/connectivity`.

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

.. test:: Connections are edited, opened and their errors notified
   :id: TEST_BLIP_CONNECTIONS
   :status: implemented
   :component: blip
   :verifies: REQ_CONN_WS, REQ_CONN_ERRORS

   ``blip/src/app/connections.rs``, ``blip/src/app/toasts.rs`` and
   ``blip/src/app.rs`` tests -- name connections after their address, add
   them with the + button, open several at once, pass on when the first one
   opens before its messages and start afresh then unless recording, keep
   them from being changed while recording but connect again those that
   broke, ask before dropping an unsaved recording, report why an invalid
   address fails to connect, show errors and warnings as toasts until they
   expire or are closed, count a repeated one instead of stacking it, and
   warn when a recording is opened in part; ``blip/src/app/model/traces.rs``
   tests -- put the samples of a path in the order of their timestamps,
   also when several connections send it.

.. test:: Samples stay on one timeline
   :id: TEST_TIMELINE
   :status: implemented
   :component: blip, outpost
   :verifies: REQ_CONN_TIMELINE

   ``blip/src/app/model.rs`` tests -- drop values and log records of another
   time base than the first sample, move records without a timestamp to a
   first timestamp in another time base, and start afresh when cleared;
   ``outpost/tests/test_timing.py`` -- keep the time of a target, shift a run
   after a jump back, a short run, or an earlier time after a new connection
   or a sequence reset to follow the newest time plus the time since, but not
   a late or a little out of order sample, shift timestamps of another clock
   alike, and change the timestamp of a message in place wherever it is in
   the map; ``test_protobuf.py`` and ``test_socket.py`` -- shift the times
   after a restart of a protobuf or socket target and those of the bridge,
   log it, keep a clock per host, and drop untimed values, also of the script,
   before the first timestamp.
