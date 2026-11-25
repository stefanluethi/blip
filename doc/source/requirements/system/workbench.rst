Library: workbench and widgets
==============================

What an application built on ``blip-kit`` can do and rely on, as the shipped
blip does (:need:`REQ_LIB_REFERENCE`).

.. req:: The session knows what is unsaved
   :id: REQ_LIB_UNSAVED
   :status: implemented
   :component: blip-kit
   :satisfies: STK_OWN_UI, STK_CONFIDENCE

   An application shall be able to ask the session whether its data holds
   samples recorded since the data was last saved, cleared or replaced, and to
   tell the session that it saved them, to ask the user before it drops them.
   Samples dropped while not recording shall not count. A source that opens
   shall not clear such data, and the logs of the new source shall not be added
   to it. The session shall also tell whether a source is connected and samples
   are dropped because nothing records them yet, so that the application can say
   so.

.. req:: Notices with a button
   :id: REQ_LIB_NOTICES
   :status: implemented
   :component: blip-kit
   :satisfies: STK_OWN_UI

   An application shall be able to show a notice at the level info, warning or
   error with a button of its own, and read which buttons were clicked, and close
   the notices of an offer that no longer applies. Saving a file shall tell the
   application the name it was saved as, and the library shall give the label of
   a Save item with an ellipsis only where a dialog asks where to save.

.. req:: The layout can be steered by key and undone
   :id: REQ_LIB_LAYOUT_UNDO
   :status: implemented
   :component: blip-kit
   :satisfies: STK_OWN_UI

   An application shall be able to delete or start renaming the selected view of
   the layout, to clear it, to undo the last delete or clear, to ask whether that
   can be undone and which delete it is, and to read the name of what was
   deleted to tell the user (:need:`REQ_LAY_UNDO`).

.. req:: The state of the connections
   :id: REQ_LIB_CONN_STATUS
   :status: implemented
   :component: blip-kit
   :satisfies: STK_OWN_UI

   An application shall be able to read in one place whether its connections
   are open, with or without a problem such as a newer protocol or another
   connection that failed, being opened, failed, or none of these, and whether
   a failure was a connection lost, and to forget the failures, e.g. once a
   recording shows in their place (:need:`REQ_CONN_STATUS`).

.. req:: Time controls and tables the application steers
   :id: REQ_LIB_TIME_API
   :status: implemented
   :component: blip-kit
   :satisfies: STK_OWN_UI

   An application shall be able to release the time cursor, to play or pause as
   its button does, to tell the time range whether a connection is open, and to
   copy the time range. A log table shall take the time range, and set its cursor
   when the user clicks a record (:need:`REQ_VIS_LOG_CURSOR`).
