Files
=====

Recordings and presets as files.

.. req:: Save and open recordings
   :id: REQ_FILE_RECORDING
   :status: implemented
   :component: blip
   :satisfies: STK_REVIEW

   blip shall save the messages recorded since the data was last cleared to
   a recording file, and open recording files, showing their data as if it
   had been received, in place of all data, traces and actions before, once the
   file is read.

.. req:: Export and import presets
   :id: REQ_FILE_PRESETS
   :status: implemented
   :component: blip
   :satisfies: STK_REVIEW

   blip shall export the presets the user chooses to one JSON file, and
   import the presets the user chooses from a file exported by the same or an
   older version of blip, replacing the presets of the same names. The import
   dialog shall mark the presets it replaces, and its button shall read "Import
   and replace" while it replaces one.

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
   in them. A recording that blip opened and saves again unchanged keeps its
   version (:need:`REQ_FILE_REOPEN`).

.. req:: Recording what arrives live
   :id: REQ_FILE_REENCODE
   :status: implemented
   :component: blip, blip-kit
   :priority: high
   :satisfies: STK_REVIEW, STK_OWN_DATA
   :rationale: Data of an application's own source never was blip-protocol bytes, and
      it must be recorded like any other.

   While recording, blip shall write what it receives to the recording as it
   arrives, whichever source it comes from and whether it arrived as bytes of the
   blip protocol or as typed data of an application, in the format of version 3
   (:need:`REQ_FILE_VERSION`). The recording shall hold every message with every
   value, in the order received, with the name of its source and with a block of
   samples still a block. A recording need not be byte for byte what was sent: it
   shall hold the old names ``Value`` and ``ValueMetadata`` as ``Sample`` and
   ``TraceMetadata``, and shall not hold the fields that blip does not know in a
   message it can read. A recording that starts after data was received shall
   begin, as before, with the latest ``Hello``, announced actions and trace
   metadata of each source.

.. req:: Shortest exact numbers in a recording
   :id: REQ_FILE_SHORTEST
   :status: implemented
   :component: blip-kit
   :priority: medium
   :satisfies: STK_REVIEW
   :rationale: A recording written again must not be longer than the messages it was
      made from, or recording would cost memory and disk.

   blip shall write the integers and floats of a recording in the shortest form
   that reads back as exactly the same value, so that a recording is not longer
   than the messages it was made from, apart from the names of the sources.

.. req:: Unreadable messages stay in a recording
   :id: REQ_FILE_UNREADABLE
   :status: implemented
   :component: blip, blip-kit
   :priority: medium
   :satisfies: STK_REVIEW
   :rationale: A newer blip may read what this one cannot, such as a message of a
      newer protocol.

   While recording, blip shall keep the messages it cannot read, such as a
   malformed block of samples or a message of a newer protocol, in the recording
   byte for byte, at their place among the others. blip shall show only what it
   can read, and notify of a malformed block as before.

.. req:: A recording opened and saved again
   :id: REQ_FILE_REOPEN
   :status: implemented
   :component: blip, blip-kit
   :priority: medium
   :satisfies: STK_REVIEW
   :rationale: Opening a file to save it elsewhere must not change it, not even where
      blip does not know what it holds.

   When the user saves a recording that blip opened, blip shall write it as it
   was opened, byte for byte, with its version and with everything in it, also
   what blip does not know.

.. req:: Ask before an unsaved recording is dropped
   :id: REQ_FILE_UNSAVED
   :status: implemented
   :component: blip, blip-kit
   :satisfies: STK_REVIEW, STK_CONFIDENCE
   :rationale: What was recorded cannot be recorded again.

   A recording shall be unsaved from its first recorded message until it is saved,
   cleared or replaced by an opened recording; samples dropped while not
   recording shall not make it unsaved. blip shall ask "Discard the unsaved
   recording?" with Discard and Cancel before Record, File › Clear recording, and
   opening or dropping a recording would drop it, and cancel on Escape or a click
   beside the question. blip shall ask "Quit and discard the unsaved
   recording?" before the native window closes, and the web page shall have the
   browser warn before it is reloaded or left. Cancelling a quit shall bring back
   a question that it replaced. Opening a connection shall not clear an unsaved
   recording, and the logs of the new connection shall not be added to it.

.. req:: Opening a recording
   :id: REQ_FILE_OPEN
   :status: implemented
   :component: blip
   :satisfies: STK_REVIEW
   :rationale: A file takes some seconds to read, during which the user can
      still change their mind.

   Opening a recording shall keep the connections and a running recording until
   the file is read, and cancelling shall leave everything as it was. Pressing
   Record while a file is read shall cancel the opening and say "Opening <file>
   was cancelled by recording", as opening a connection does when none was
   active before. While a file is dragged over the window, blip shall show "Drop
   to open". The name of the opened file shall show in the connections chip and
   in the title of the native window until other data replaces it. Opening a
   recording shall clear the failures shown of the connections.

.. req:: Tell that a recording is saved
   :id: REQ_FILE_SAVED
   :status: implemented
   :component: blip, blip-kit
   :satisfies: STK_REVIEW, STK_CONFIDENCE

   blip shall tell "Saved <name>" once a recording is saved, and "Nothing
   recorded to save" when saving with nothing recorded. On the web the items
   Save and Export shall have no ellipsis, as they download at once; natively
   they shall have one, as a dialog asks where to save.
