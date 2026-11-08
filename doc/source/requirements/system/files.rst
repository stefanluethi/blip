Files
=====

Recordings and presets as files.

.. req:: Save and open recordings
   :id: REQ_FILE_RECORDING
   :status: implemented
   :component: blip
   :satisfies: STK_REVIEW

   blip shall save the messages received since the data was last cleared to
   a recording file, and open recording files, showing their data as if it
   had been received, in place of all data, traces and actions before.

.. req:: Export and import presets
   :id: REQ_FILE_PRESETS
   :status: implemented
   :component: blip
   :satisfies: STK_REVIEW

   blip shall export the presets the user chooses to one JSON file, and
   import the presets the user chooses from a file exported by the same or an
   older version of blip, replacing the presets of the same names.
