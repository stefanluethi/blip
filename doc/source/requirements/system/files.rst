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

.. req:: Recording file version 3
   :id: REQ_FILE_VERSION
   :status: implemented
   :component: blip, outpost, sonde
   :satisfies: STK_REVIEW
   :rationale: A recording with the new messages must not be taken for one an
      older reader can understand.

   blip, the outpost and sonde shall write recording files of format version
   3, as they write the new messages (``Hello``, ``Sample``, ``Samples`` and
   ``TraceMetadata``). The layout of version 3 shall be that of version 2 for
   blip, with the names of the connections between the messages, and that of
   version 1 for the outpost and sonde, without names, gzip-compressed by the
   outpost and uncompressed by sonde. blip and the outpost shall read the
   versions 1 to 3 and the outpost shall replay the messages of a recording
   of any of them. blip shall read the old names ``Value`` and ``ValueMetadata``
   in them.
