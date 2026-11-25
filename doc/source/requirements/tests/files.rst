Files
=====

Tests of the requirements in :doc:`/requirements/system/files`.

.. test:: Recordings are saved and opened
   :id: TEST_BLIP_RECORDING
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_RECORDING

   ``kit/src/store/recording.rs`` and ``blip/src/app.rs`` tests -- save
   messages and read them back with every value, read uncompressed files, files of
   several gzip members and files written by the Python outpost, read a cut
   off file up to the cut and reject other files; open a recording in place
   of the data and units before, and with the traces panel started anew. The
   recordings of before the typed entry (kept as test data) open with the same
   data as before.

.. test:: Presets are exported and imported as JSON
   :id: TEST_BLIP_PRESETS_FILE
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_PRESETS

   ``kit/src/workbench/layout/file.rs::layouts_survive_json`` -- saves a layout
   with a view of each type as JSON and checks it loads unchanged.
   ``kit/src/workbench/presets.rs`` tests -- write presets with their layout and
   connections to a file and read them back unchanged, export the chosen
   presets, import replacing those of the same names, refuse files of a newer
   version, layout files and presets without or with the same names, keep the
   connections read apart, refuse an import while a dialog is open, leave out
   the active preset while recording and take it once recording stops, keep
   kept presets that cannot be read, read the demo presets.
   ``blip/src/app.rs::a_presets_file_is_imported_as_chosen_in_its_dialog`` --
   opens a presets file, imports it from its dialog and shows the replaced
   active preset.

.. test:: blip writes and reads recording version 3
   :id: TEST_BLIP_RECORDING_V3
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_VERSION

   ``kit/src/store/recording`` unit tests: ``recordings_are_written_as_version_3``
   checks that blip writes the header with version 3 and the names of the
   connections as in version 2; ``recordings_of_versions_1_to_3_are_read`` that it
   reads recordings of versions 1, 2 and 3, with ``Value`` and ``ValueMetadata``
   of the old ones and ``Hello``, ``Sample``, ``Samples`` and ``TraceMetadata``
   of the new, and ``other_files_are_no_recordings`` that it refuses a version
   above 3. That an opened recording of version 1 or 2 keeps its version when
   saved again is checked by :need:`TEST_BLIP_RECORDING_REOPEN`.

.. test:: The outpost writes, reads and replays recording version 3
   :id: TEST_OUTPOST_RECORDING_V3
   :status: implemented
   :component: outpost
   :verifies: REQ_FILE_VERSION

   ``outpost/tests/test_recording.py::test_the_outpost_writes_version_3_without_connection_names``
   checks the layout a blip opens: the gzip magic, the header
   ``{"format": "blip recording", "version": 3}``, only maps after it and no
   connection names. ``test_versions_1_to_3_are_read`` (compressed or not),
   ``test_the_connection_names_of_recordings_by_blip_are_skipped`` (versions 2
   and 3) and ``test_a_newer_version_is_refused`` check that the outpost reads
   versions 1 to 3 without the names of blip and refuses version 4.
   ``test_the_messages_of_every_version_are_replayed_unchanged`` replays a
   file of each version, with the old names in version 1 and a ``Hello`` and
   ``Samples`` blocks in versions 2 and 3, and checks that a client receives
   and the new recording holds the frames byte for byte.
   ``test_recordings_of_sonde_are_read`` reads the recording of sonde.

.. test:: sonde writes recording version 3
   :id: TEST_SONDE_RECORDING_V3
   :status: implemented
   :component: sonde
   :verifies: REQ_FILE_VERSION

   ``sonde/tests/test_file_sink.cpp`` ("a file sink writes a recording",
   "the recording fixture is what sonde writes") -- checks that ``FileSink``
   writes the header ``{"format": "blip recording", "version": 3}``
   uncompressed, then the batches as whole messages (the items decode one by
   one), beginning with the ``Hello`` of the queue, and that the fixture ``sonde/tests/data/recording.blip`` is what sonde
   writes. ``proto/tests/sonde.rs`` and ``outpost/tests/test_recording.py``
   read the fixture.

.. test:: What arrives live is recorded without loss
   :id: TEST_BLIP_RECORDING_LIVE
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_FILE_REENCODE, REQ_FILE_UNREADABLE

   ``kit/src/store/session/tests.rs::what_arrives_live_is_recorded_without_loss``,
   ``kit/src/store/recording/tests.rs::unreadable_messages_are_recorded_as_they_are`` -- a session records messages of two sources, one handing over bytes
   of the protocol and one typed data, with a ``Hello``, trace metadata, actions,
   the result of an action, log records, samples of every kind of value (also
   those a trace cannot hold, such as an array of strings and records in arrays),
   and blocks with a period and with timestamps, and the old names ``Value`` and
   ``ValueMetadata``. The test saves the recording, reads it back and checks that
   it holds every message with every value, in the order received, with the
   names of the sources and with each block still a block, that the old names
   are written as the new ones, that the header and the version 3 are as
   before, and that a recording that starts after data was received begins with
   the latest ``Hello``, actions and trace metadata of each source. A frame with
   a malformed block of samples among good messages, and a message of a newer
   protocol, are in the recording byte for byte at their place, while the store
   drops the block with the notice as before.

.. test:: Numbers are written in their shortest exact form
   :id: TEST_BLIP_RECORDING_SHORTEST
   :status: implemented
   :component: blip-kit
   :verifies: REQ_FILE_SHORTEST

   ``kit/src/store/cbor/tests.rs`` (``numbers_are_written_in_their_shortest_exact_form``,
   ``every_half_precision_float_is_written_as_one``,
   ``floats_read_back_as_the_same_value``),
   ``kit/src/store/protocol/tests.rs::the_writer_writes_what_serde_writes`` and
   ``kit/src/store/recording/tests.rs::a_recording_is_not_longer_than_the_messages_it_was_made_from``
   -- records integers of each size and floats that are exact in half,
   single and double precision, and infinity, negative zero and NaN, and checks
   that each is written in the shortest form that reads back as the same value
   (a NaN as a NaN), that the file is not longer than the messages it was made
   from, apart from the names of the sources, for messages written with every
   float in double precision and for messages already in the shortest form, and
   that a block of floats does not grow.

.. test:: An opened recording is saved as opened
   :id: TEST_BLIP_RECORDING_REOPEN
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_FILE_REOPEN

   ``kit/src/store/recording/tests.rs::an_opened_recording_is_saved_as_it_was_opened``
   and ``a_recording_after_opening_starts_with_the_descriptions_of_the_file`` --
   opens recordings of versions 1, 2 and 3, compressed and not, one
   with the old names, one with a field blip does not know, one with a message of a
   newer protocol and one written by the outpost, saves each again and checks that
   the bytes are those of the file opened.
   ``blip/src/app/tests.rs::a_reopened_recording_is_saved_as_it_was_opened`` --
   a file of version 2 opened in the app is saved again as the bytes opened, from
   the recorder of the session.
