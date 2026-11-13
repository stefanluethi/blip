Protocol
========

Tests of the requirements in :doc:`/requirements/system/protocol`.

.. test:: Messages are named Sample and TraceMetadata
   :id: TEST_PROTO_NAMES
   :status: implemented
   :component: proto, blip, outpost, sonde
   :verifies: REQ_PROTO_NAMES

   ``proto/tests/messages.rs`` ``samples_and_metadata_have_the_new_names``
   checks that ``proto`` encodes a sample and trace metadata as
   ``Sample`` and ``TraceMetadata`` and decodes both these and the old
   ``Value`` and ``ValueMetadata``. ``blip/src/app/model/decode`` unit tests
   (``old_and_new_names_decode_alike``) check that blip reads the old names as the
   new ones, and ``recordings_of_versions_1_to_3_are_read`` in
   ``blip/src/app/model/recording`` also in a recording of version 1 or 2.
   ``proto/tests/sonde.rs`` checks that the fixture sonde wrote decodes as
   ``Sample`` and ``TraceMetadata`` and encodes again byte for byte.
   Outpost: ``outpost/tests/test_proto.py``
   ``test_values_and_metadata_are_sent_as_sample_and_trace_metadata`` and
   ``test_prefixes_tell_the_messages_apart`` check that the outpost sends
   ``Sample`` and ``TraceMetadata`` (no ``Value``) and that the prefixes tell new
   and old names apart; ``test_metadata_path`` checks reading the path of both
   metadata names; ``outpost/tests/test_server.py``
   ``test_the_metadata_of_each_path_is_kept_under_either_name`` checks the cache
   under either name; ``outpost/tests/test_socket.py``
   ``test_old_names_of_a_target_are_forwarded_unchanged_and_the_metadata_is_kept``
   and ``test_a_restart_shifts_the_timestamps_of_an_old_named_value`` check that
   old names are forwarded byte for byte and retimed;
   ``test_log_levels_are_those_of_the_protocol`` checks the levels 0 to 4.
   sonde: ``sonde/tests/test_encoding.cpp`` ("sonde writes the new names and no
   old one", "samples are the messages of proto", "a unit is trace metadata")
   checks that a value is a ``Sample`` and a unit ``TraceMetadata``, byte for
   byte as proto encodes them, and that no ``Value`` or ``ValueMetadata`` is
   written.

.. test:: Time base follows the clock or the guess
   :id: TEST_BLIP_TIME_BASE
   :status: implemented
   :component: proto, blip
   :verifies: REQ_PROTO_TIME

   ``blip/src/app/model`` unit tests (``a_declared_clock_decides_the_time_base``)
   and ``blip/src/app/time`` unit tests -- check that a connection with a ``Hello`` of the clock ``epoch``
   or ``boot`` has the time base of its clock whatever its timestamps are,
   that without a ``Hello`` timestamps before 2001-09-09 are the time since boot
   and later ones the Unix epoch, that timestamps are taken as
   nanoseconds, and that the time range takes the base of the model.

.. test:: Log records at 0 are placed and levels above 4 are errors
   :id: TEST_BLIP_LOG_CONTRACT
   :status: implemented
   :component: blip
   :verifies: REQ_PROTO_LOG_NOW, REQ_PROTO_LEVELS

   ``blip/src/app/model/logs`` unit tests
   (``records_at_0_are_placed_and_levels_above_4_are_errors``) and
   ``blip/src/app/model`` unit tests (``a_record_at_0_follows_the_newest_data``,
   ``a_record_at_0_without_data_shows_the_time_of_day``)
   -- check that a record with the timestamp 0 is placed at the
   newest time of the data, or at the time of day without data; that levels 0
   to 4 are Trace, Debug, Info, Warn and Error; and that a level above 4 is
   shown as Error.

.. test:: Trace metadata round-trips and applies to field and array traces
   :id: TEST_PROTO_METADATA
   :status: implemented
   :component: proto, blip
   :verifies: REQ_PROTO_METADATA

   ``proto/tests/messages.rs`` ``trace_metadata_round_trips``,
   ``integer_range_ends_widen`` and ``a_label_key_out_of_range_fails`` check
   that ``TraceMetadata`` with and without description, labels and range
   round-trips (also byte for byte), that one with only a path, as older senders
   send it, still decodes, that integer range ends decode as floats and that a
   label key outside ``i64`` fails the message.
   ``blip/src/app/model/traces`` unit test
   ``metadata_applies_to_fields_and_elements`` checks that blip applies the
   metadata of a path to the trace at it, to a record field trace (``imu.acc``)
   and to the elements of an array trace (``imu.acc[0]``), drops an unusable
   range, removes the entry for metadata with only a path and keeps it when the
   samples are cleared.

.. test:: Hello round-trips
   :id: TEST_PROTO_HELLO
   :status: implemented
   :component: proto
   :verifies: REQ_PROTO_HELLO

   ``proto/tests/messages.rs`` ``hello_round_trips``,
   ``a_hello_with_sorted_keys_decodes``,
   ``an_unknown_clock_is_none_and_a_missing_version_fails`` and
   ``the_version_and_levels`` check that ``Hello`` with the version 1, a name,
   clock and sender round-trips, with both clocks and byte for byte, that keys in
   another order and an unknown clock decode, and that a missing version fails.
   ``proto/tests/sonde.rs`` (``decodes_a_recording_of_sonde``) checks that blip
   decodes the ``Hello`` that sonde encodes.
   ``outpost/tests/test_outpost.py`` ``test_a_client_gets_a_hello_first`` checks
   the ``Hello`` the outpost sends (version 1, its name, clock and sender), whose
   key order ``a_hello_with_sorted_keys_decodes`` covers in proto.

.. test:: Blocks of samples are well-formed
   :id: TEST_PROTO_SAMPLES
   :status: implemented
   :component: proto
   :verifies: REQ_PROTO_SAMPLES

   ``proto/tests/samples.rs`` ``blocks_round_trip_in_both_forms``,
   ``malformed_blocks_are_refused`` and ``times_of_the_samples`` check that
   ``Samples`` round-trips in both forms, with a period and with timestamps (byte
   for byte), that decoding rejects one with both or neither, one with only a
   timestamp or only a period, one whose values and timestamps differ in length
   and one whose timestamps pass 2^64 ns, and that the block gives its single
   samples.

.. test:: Blocks of samples give the traces of single samples
   :id: TEST_BLIP_SAMPLES
   :status: implemented
   :component: blip
   :verifies: REQ_PROTO_SAMPLES_APPLY, REQ_VIS_PLOT

   ``blip/src/app/model`` unit tests: ``blocks_give_the_traces_of_single_samples``
   checks that a block with a period and one with timestamps, of numbers with a
   null and of records, give the same traces as the equivalent ``Sample``
   messages; ``a_malformed_block_is_dropped_alone`` that a malformed block is
   dropped whole with one error notice, also from a source without a name, while
   the other messages of its frame are kept; ``an_empty_block_gives_nothing``
   and ``the_latest_value_of_a_block_is_its_last`` the empty block and the
   readback of a block. ``blip/src/app/model/decode`` unit tests
   (``blocks_are_read_sample_by_sample``, ``malformed_blocks_are_refused``) check
   the two-pass reader and the texts of the refused blocks.
   ``a_plot_shows_blocks_as_single_samples`` in ``blip/src/app/view/plot`` checks
   that a plot shows the samples of a ``Sample`` and of a ``Samples`` message
   alike.
