Time range and recording
========================

All views show the same time range, set in the bottom panel. It holds the
**Record** button, play and pause, **Live**, the length of the window and the
timeline of all collected data.

.. figure:: /screenshots/panel-time.png
   :target: ../_images/panel-time.png
   :class: screenshot
   :alt: The bottom panel while recording live: Record, pause, Live, the window length and the timeline with its bars and thumb.

   The bottom panel while recording.

.. figure:: /diagrams/time-states.svg
   :target: ../_images/time-states.svg
   :class: diagram
   :alt: Live goes to paused on pause, dragging a plot, moving the thumb or
         setting the cursor; paused goes to playing on play; playing goes back
         to live once it reaches the newest data; Live returns from paused.

   How the time range moves.

Recording
---------

**Record** keeps the values and log records received from the connections.

* While it is off they are dropped, as if the connections were closed, but the
  connections stay open: the targets' units and actions are still kept and
  actions can still be invoked.
* A recording starts only while a connection is open. Starting it clears the
  data as **File › Clear recording** does and follows the newest data live.
  Press :kbd:`R` instead of clicking the button.
* While a connection is open, blip does not record and has no data, the
  **Record** dot is amber and says on hover that the incoming data is dropped
  until you press Record; the empty timeline says *Press Record to collect
  data*. Without a connection it says *Connect to an outpost or open a
  recording*, and while recording without data *Waiting for data*.
* Stopping it keeps the data, also when the connections close.
* While recording, the connections cannot be changed, but a broken one can be
  connected again, see :doc:`/blip/connecting`.

blip starts without recording, as does opening a recording file. Save what was
recorded with **File › Save recording**, see :doc:`/blip/files`.

A recording is **unsaved** from its first recorded message until you save it,
clear it or open another. blip asks *Discard the unsaved recording?* before
**Record** starts anew, before **File › Clear recording**, before opening or
dropping a recording and when you quit, see :doc:`/blip/files`.

Live, pause and play
--------------------

* **Live** (:kbd:`L`) follows the newest data, playing, while recording. In an
  opened recording, and whenever the time cursor is set, it is also available:
  it shows the end of the data (*Follow the end of the recording*) and
  releases the cursor.
* **Pause** stops the range where it is, while recording goes on, to inspect
  the data.
* **Play** (:kbd:`Space` toggles play and pause) moves the range on from
  there at the pace of the timestamps, along with the time cursor, and
  returns to live mode once it reaches the newest data. It shows unless
  something moves; it is disabled without data and at the end of a recording.
* While recording and not live, the bar says how far the range is behind, e.g.
  *Paused, 12 s behind*.
* The field next to them sets the length of the range (the **window**), e.g.
  ``500 ms``, ``20 s`` or ``5 min``. The caret at its right end opens a menu
  of round lengths from 10 ms to 1 h. The field reads the units ``us`` or
  ``µs``, ``ms``, ``s`` or ``sec``, ``m`` or ``min``, ``h`` or ``hr`` and
  ``d``, in any case. Text it cannot read stays in the field, outlined in the
  error colour; hover it for the forms it accepts. A length that is too short
  or too long for the data is limited and marked too, and its hover says the
  least and the most.

Plots move the range too: dragging a plot sideways moves it and pauses, and
scrolling the mouse wheel over a plot zooms it, which changes the window and
keeps following live data.

The timeline
------------

The timeline shows all collected data; its **thumb** shows the range.

* Drag the thumb to move the range, drag its left or right edge to change where
  the range starts or ends, or click the timeline to center the range there.
  Moving the range or its end pauses; moving it to the end of the data returns
  to live mode.
* Scrolling the mouse wheel over the timeline zooms the timeline itself, to set
  the range more precisely, and scrolling sideways (or with :kbd:`Shift`)
  moves it. Zoomed in at the end of the data, it moves on with the newest
  data. The button at the end of its ruler shows all data again. Showing all
  data, the timeline rescales in steps as data arrives, and not at all while
  it is pressed.
* The **ruler** above it shows the times. Hovering it shows the time at the
  pointer, with the date in the pill; the first label of a new day has the
  date too. Pressing or dragging on it sets the time cursor, and the range
  moves with it.
* The **bars** show what arrived in round steps of time: the log records at
  the bottom in the colours of their levels, the most severe lowest, and the
  values in grey above them, the fields of a record counted once. Hover the
  log records of a bar to list them; click them to put the cursor at the most
  severe one.
* **Hatched stretches** are gaps: nothing arrived for at least ten times the
  mean time between samples, e.g. while disconnected. Hover one for *No data
  for* and its duration.

The time cursor
---------------

The time cursor is a vertical line with a handle on top, shown in all plots
over time and on the timeline. The table, the heatmap at the cursor and the
markers of the XY view show the values at its time.

It stays at the end of the range until it is set, drawn faint in the plots
there: click a plot to put it there, drag it in a plot, press or drag on
the ruler of the timeline or click an entry of a log view.
Setting it pauses, and it stays set until you release it with :kbd:`Escape`
(when nothing else takes the key) or press **Live**, which returns the cursor
to the end of the range.

Time axes
---------

The time axes show the timestamps as local time of day, or as time since boot.
Which one follows the ``clock`` in the ``Hello`` of the connection, ``epoch`` or
``boot``; a target without it is guessed: timestamps too small to be a date
(before 2001) are the time since boot. All data shares one axis: if
connections declare both clocks, the time of day wins and the data of the time
since boot is removed, with one warning. Log records with the timestamp 0 are
placed at the newest time of the data.

Next steps
----------

* :doc:`/blip/files` -- save the recording.
* :doc:`/blip/controls` -- the keyboard shortcuts.
* :doc:`/blip/actions` -- carry out actions while recording.
