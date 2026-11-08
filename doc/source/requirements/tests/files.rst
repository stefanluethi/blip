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
