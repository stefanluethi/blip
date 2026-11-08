Time
====

The time cursor, recording and playback.

.. req:: Time cursor
   :id: REQ_VIS_CURSOR
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall show one time cursor in all views over time, which the user can
   set and drag, and which stays at the end of the shown time range until set.

.. req:: Record, pause and play
   :id: REQ_VIS_PLAYBACK
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE, STK_REVIEW

   blip shall keep the received values and log records only while recording,
   which the user switches on and off without closing the connections. A
   recording shall start only while a connection is open, and starting it
   shall clear the data received before. blip shall let the user pause the
   shown time range and play it from there at the pace of the timestamps
   while recording goes on.
