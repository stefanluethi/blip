Typed store entry
=================

Tests of the requirements in :doc:`/requirements/system/ingest`. Their locations
are filled in by the developer of the change.

.. test:: A store takes typed data
   :id: TEST_LIB_INGEST_TYPED
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LIB_INGEST_TYPED, REQ_LIB_SESSION

   ``kit/src/store/session/tests.rs::a_store_of_the_application_takes_typed_data``
   -- a store of the test that is not the in-memory one takes the typed
   entry and keeps what it is given as owned values. A session of the test feeds
   it from a source that hands over typed data of each kind: a sample of each
   kind of value (none, integer, float, bool, string, arrays of one and of two
   dimensions with gaps, nested records, an array of strings), a block of
   samples with a period and one with timestamps, a log record with and without a
   path, trace metadata, announced actions and a clock. The test checks that the
   store holds each as given, in order, and that the session records, hands the
   result of an action to the invoker and answers the queries of the store,
   with no CBOR in the path from the source to the store (checked by review of the
   modules the typed path uses).

.. test:: Bytes and typed data give the same
   :id: TEST_LIB_INGEST_SAME
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LIB_INGEST_SAME

   ``kit/src/store/session/tests.rs::the_parser_and_the_typed_way_agree``
   -- two sessions with a test store that keeps what it is given. A list of
   messages with one of each kind (a ``Hello`` with each clock and with none,
   trace metadata with and without more than the path, actions, the result of an
   action as ok and as an error, a log record with and without a path, a sample of
   every kind of value, a block with a period and one with timestamps, and the old
   names ``Value`` and ``ValueMetadata``, whose bytes are compared with the new
   names) goes to the first as typed data and to the second as the bytes of the
   protocol. The test checks that both stores got the same, that the state of the
   invoker and the notices are the same, and that the recordings of the two are
   byte for byte equal, once while recording and once while not (where the stores
   get the latest value of each path).

.. test:: Samples are not kept while not recording, the latest value is
   :id: TEST_LIB_INGEST_LATEST
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LIB_INGEST_LATEST

   ``kit/src/store/session/tests.rs::samples_are_not_kept_while_not_recording_but_the_latest_value_is``,
   ``kit/src/store/protocol/tests.rs`` (``samples_are_read_only_up_to_their_latest_value_while_not_recording``,
   ``a_block_is_read_only_up_to_its_last_value_while_not_recording``) -- a source hands over samples, blocks and log records while the
   session does not record, once as bytes and once as typed data with the same
   code of the source for recording and not. The test checks that the store holds
   no samples and no log records, that the latest value of each path (the last of
   a block) shows in the readbacks of an announced action, that a value that
   arrived before its path became a readback shows after the action is announced,
   and that recording then starts from what was received, as before. The reading
   of samples only up to their latest value is measured by the rate of taking in
   samples while not recording (:need:`TEST_LIB_STORE_MEMORY`).

.. test:: The internals of the store are not public
   :id: TEST_LIB_INGEST_PRIVATE
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LIB_INGEST_PRIVATE

   ``kit/tests/own_view.rs`` (``a_view_of_the_application_follows_the_time_of_the_plot_beside_it``,
   ``the_lookups_of_a_series_and_a_trace_find_samples_by_time``), ``check.sh`` -- ``kit/tests/own_view.rs``, an integration test outside the
   crate, writes a view that finds the sample at and closest to a time and the
   samples in a range with the lookups of the series and trace types and nothing
   else; it compiles and passes. The documentation of the crate that ``check.sh``
   builds lists no public module of the CBOR reader, the decoder, or the tables that
   grow and stay sorted (checked by review of the public modules of ``kit/src/lib.rs``
   and of the store).

.. test:: The user of blip sees no change by the typed entry
   :id: TEST_LIB_INGEST_NO_CHANGE
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_LIB_INGEST_NO_CHANGE

   ``kit/src/store``, ``kit/src/widgets``, ``kit/src/workbench`` and
   ``blip/src/app/tests.rs`` tests, ``kit/tests`` -- the tests of the store, the
   actions, the widgets, the timeline, the session, the workbench and
   ``blip/src/app/tests.rs`` pass with only the way they hand data over adapted
   (typed messages and inputs, or a session's inbox), not what they check; ``make screenshots`` renders
   the screenshots of the documentation, compared with those before the change by
   hand with ``make screenshots SCREENSHOTS_DIR=…``.

.. test:: Connections are sources of the session
   :id: TEST_LIB_CONN_SOURCES
   :status: draft
   :component: blip, blip-kit
   :verifies: REQ_LIB_CONN_SOURCES

   ``TODO`` -- connections of the test to a WebSocket server of the test deliver
   samples, a log record, a ``Hello`` and the result of an action. The test checks
   that they reach the store, the recording and the invoker through the session
   alone, as those of a source of the test do, that the connections poll and
   split nothing themselves, and that one connection opening first starts afresh
   and a later one does not.

.. test:: Sources of the same name are told apart
   :id: TEST_LIB_SOURCE_SAME_NAME
   :status: draft
   :component: blip-kit
   :verifies: REQ_LIB_SOURCE_SAME_NAME

   ``TODO`` -- two sources of the test with the same name, and two connections
   with no name, are added to a session. The test checks that the data and the
   status of each are its own, that a request goes to the source it is meant for,
   that the notice of a failure names the source that failed, and that closing,
   failing or removing one leaves the data and the status of the other as they
   are.

.. test:: The user sees no change by connections as sources
   :id: TEST_LIB_CONN_NO_CHANGE
   :status: draft
   :component: blip
   :verifies: REQ_LIB_CONN_NO_CHANGE

   ``TODO`` -- the tests of the connections, of the ``Hello`` in the popup, of the
   newer protocol and of the notices (:need:`TEST_BLIP_CONNECTIONS`,
   :need:`TEST_BLIP_HELLO`, :need:`TEST_BLIP_HELLO_NEWER`) pass with only the way
   they poll the connections adapted, not what they check; the screenshots of the
   connections popup are compared with those before the change by hand.
