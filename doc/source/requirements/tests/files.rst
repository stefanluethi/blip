Files
=====

Tests of the requirements in :doc:`/requirements/system/files`.

.. test:: Recordings are saved and opened
   :id: TEST_BLIP_RECORDING
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_RECORDING

   ``blip/src/app/model/recording.rs`` and ``blip/src/app.rs`` tests -- save
   frames and read them back unchanged, read uncompressed files, files of
   several gzip members and files written by the Python outpost, read a cut
   off file up to the cut and reject other files; open a recording in place
   of the data and units before, and with the traces panel started anew.

.. test:: Presets are exported and imported as JSON
   :id: TEST_BLIP_PRESETS_FILE
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_PRESETS

   ``blip/src/app/layout/file.rs::layouts_survive_json`` -- saves a layout
   with a view of each type as JSON and checks it loads unchanged.
   ``blip/src/app/presets.rs`` tests -- write presets with their layout and
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

   ``blip/src/app/model/recording`` unit tests: ``recordings_are_written_as_version_3``
   checks that blip writes the header with version 3 and the names of the
   connections as in version 2; ``recordings_of_versions_1_to_3_are_read`` that it
   reads recordings of versions 1, 2 and 3, with ``Value`` and ``ValueMetadata``
   of the old ones and ``Hello``, ``Sample``, ``Samples`` and ``TraceMetadata``
   of the new, and ``other_files_are_no_recordings`` that it refuses a version
   above 3.

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
