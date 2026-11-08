Time
====

Tests of the requirements in :doc:`/requirements/system/time`.

.. test:: Time cursor is set and dragged
   :id: TEST_BLIP_CURSOR
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_CURSOR

   ``blip/src/app/time.rs``, ``blip/src/app/view/time_axis.rs`` and
   ``blip/src/app/model/sorted.rs`` tests --
   keep the cursor at the end of the range until set, stop live mode when it
   is set, and click and drag it in a plot without panning the plot.

.. test:: Recording is switched, the time range paused and played
   :id: TEST_BLIP_PLAYBACK
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_PLAYBACK

   ``blip/src/app.rs``, ``blip/src/app/time.rs``,
   ``blip/src/app/view/time_axis.rs``, ``blip/src/app/model/sorted.rs`` and
   ``blip/src/app/model.rs`` tests --
   drop values and log records while not recording but keep units,
   actions and answers, start a recording only while connected, clearing the
   data but the latest units and actions, which are recorded again, stop
   recording when a recording file is opened, keep a paused range while data
   arrives, play on from it with the cursor without skipping ahead by the
   paused time, return to live mode at the newest data, pause when panning or
   setting the cursor but not when zooming, also with the mouse wheel over a
   plot, stay unchanged when zooming beyond the shortest or longest window, and
   switch with the buttons of the bottom panel.
