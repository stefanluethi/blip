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
   of the data and units before, and with the traces panel started anew
   (``opening_a_recording_stops_recording_and_starts_afresh``). The
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
   ``blip/src/app/tests.rs::a_presets_file_is_imported_as_chosen_in_its_dialog`` --
   opens a presets file, imports it from its dialog, whose button reads "Import
   and replace" while the active preset is replaced, and shows the replaced
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

.. test:: An unsaved recording is asked about
   :id: TEST_BLIP_UNSAVED
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_FILE_UNSAVED, REQ_VIS_PLAYBACK

   ``kit/src/store/session/tests.rs::recorded_samples_are_unsaved_until_saved_cleared_or_replaced``
   -- samples dropped while not recording are not unsaved, recorded ones are
   until saved, and again after more is recorded, and clearing or replacing the
   data ends it. ``blip/src/app/tests.rs`` --
   ``clearing_asks_before_it_drops_an_unsaved_recording`` checks the question
   "Discard the unsaved recording?", that Escape and Cancel keep the data,
   Discard drops it, and that nothing is asked with nothing unsaved;
   ``record_asks_before_it_drops_an_unsaved_recording`` that Record asks while
   connected and stopping never asks; ``the_record_key_asks_like_the_button``;
   ``shortcuts_wait_while_a_question_is_open``;
   ``a_cancelled_quit_asks_the_question_before_it_again`` that the quit
   question comes first and the one it replaced comes back on Cancel;
   ``a_recording_is_opened_across_frames_and_can_be_cancelled`` that opening
   over an unsaved recording asks first. The warning of the browser on reload
   and the quit of the native window are checked by hand.

.. test:: A recording is opened across frames and can be cancelled
   :id: TEST_BLIP_OPEN
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_OPEN

   ``blip/src/app/tests.rs`` --
   ``a_recording_is_opened_across_frames_and_can_be_cancelled`` checks that the
   data shown stays until all of the file is read, that connecting while it is
   read cancels it with "cancelled by connecting", and that cancelling leaves the
   connections, the running recording and the data as they were;
   ``recording_cancels_a_recording_being_opened`` that Record cancels it with
   "cancelled by recording" and a recording that went on first does not;
   ``an_opened_recording_is_named_in_the_chip_until_other_data_replaces_it``
   that the chip names the file until the data is cleared;
   ``file_names_drop_their_folders``; ``a_recording_opened_in_part_is_toasted``.
   ``kit/src/workbench/connections/tests.rs::clearing_the_failures_leaves_no_status``
   checks that the failures can be forgotten once the file shows. The overlay
   "Drop to open" and the title of the native window are checked by hand.

.. test:: Saving is told
   :id: TEST_BLIP_SAVED
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_FILE_SAVED

   ``blip/src/app/tests.rs::saving_nothing_tells_so`` -- Ctrl+S with nothing
   recorded shows "Nothing recorded to save" as a neutral notice.
   ``kit/src/workbench/files/tests.rs::a_save_label_has_an_ellipsis_only_where_a_dialog_follows``
   -- the label has the ellipsis natively and none on the web.
   ``kit/src/workbench/toasts/tests.rs::a_toast_is_shown_until_it_is_closed``
   -- a neutral notice such as "Saved a.blip" is shown. The name a native or web
   save returns is checked by hand.

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
