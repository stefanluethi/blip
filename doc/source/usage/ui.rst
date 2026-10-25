Using the UI
============

Connecting
----------

The **Connections** panel below the properties lists the outposts and targets
by their name and WebSocket address, starting with
``ws://127.0.0.1:3000/ws/outpost`` of an outpost started with its defaults, named
``outpost``. Several of them can be connected at once.

* Add an address with the **+** button, Enter keeps it, Escape drops it. The
  new connection is named after the last part of the path of the address,
  e.g. ``robby`` for ``ws://192.168.1.10/ws/robby``, numbered if another
  connection has the name.
* Click a connection to connect it, click it again to disconnect it. The
  connections are independent: connecting another one keeps the data
  received so far, clear it with **File > Clear data**. The received values
  are kept only while recording, started with **Record** in the bottom panel
  once connected, see `Time range`_.
* A connection that opens while no other is open starts afresh: the data,
  traces and actions received before or of an opened recording are dropped,
  also the units and actions **Clear data** keeps, and the traces panel
  starts anew, see `Window`_. While a recording is not saved, connecting asks
  first.
* Right-click a connection to connect or disconnect it, to rename it, to edit
  its address or to delete it. Rename it while it is disconnected.
* While recording, the connections cannot be changed: they cannot be
  connected, disconnected, added, edited or deleted. Stop recording to change
  them. Only a connection that was connected when the recording started and
  broke can be connected again, and the recording goes on.

The paths of the traces, the components of the logs and the paths of the
actions are shown as the outposts send them, e.g. ``cpu/load``, without the
name of the connection: organizing them is up to the user, e.g. by the paths
each outpost sends. Traces of the same path from several connections are one
trace, its samples in the order of their timestamps, as are the log records.
Several outposts may announce an action at the same path, an actions
view sends it to the outpost chosen in its properties, see `Actions`_. A saved
recording keeps the name of the connection of each message. The names of two
connections differ, unless they are empty.

Each target sends its own timestamps. Targets sending the time since boot
rather than the time of day are shown next to each other only as far as they
were started at the same time.

The dot before each connection shows its state: hollow when disconnected,
a spinner while connecting, green when connected and red when connecting
failed or the connection broke. Hover the red dot for the reason, which is
also shown as a notice, see `Window`_, as is a connection closed by its
outpost or target.

Window
------

Top panel
   The menu, see `Menu`_.

Left panel
   The **Traces & actions** of the selected view, as a tree that branches at
   each ``/`` of the path, with the type of each trace and a lightning for each
   announced action, and the **Layout** of the views below. Drag the border
   between them to resize them. The checkboxes choose the traces a view shows,
   or the actions of an actions view; traces cannot be checked for an actions
   view, nor actions for other views. The fields of a
   record are separate traces joined to its path with ``.``, e.g.
   ``imu.acc``, listed below the record as ``.acc``. Arrays list their
   elements, e.g. ``imu.acc[0]``, if they have at most 16; larger arrays
   cannot be plotted yet. The search field above the tree shows only the
   traces and actions whose path contains each of its words, ignoring case.
   The filter button beside it shows only the traces and actions the selected
   view can show, e.g. the numbers for a plot or the actions for an actions
   view. It stays on until it is clicked again, and is disabled while no view
   that shows traces or actions is selected, which shows all of them.
   When a recording file is opened and when a connection opens while no other
   is open, the traces panel starts anew: the search is cleared, the tree is
   scrolled to the top and its branches are open.

Right panel
   The **Properties** of the selected view or group, see `Properties`_, and
   the **Connections** below, see `Connecting`_. Drag the border between them
   to resize them.

Center
   The views, laid out as edited in the layout panel or by dragging them in
   the center, see `Layout`_. Errors, e.g. when
   connecting fails or a file cannot be opened or saved, and warnings, e.g.
   when a recording file is cut off, show as notices stacked in its bottom
   right corner. They disappear after a few seconds, later while the pointer
   rests on them, or when closed with their **×** button. An error that
   repeats while it is shown is counted instead of shown again.

Bottom panel
   The time range shown by all views.

Menu
----

File
   * **Open recording** (:kbd:`Ctrl+O`) disconnects all connections, stops
     recording and shows the data of a recording file as if it had been
     received, in place of all data, traces and actions before, with the
     time range fitted to all of it and the traces panel started anew.
   * **Save recording** (:kbd:`Ctrl+S`) saves all messages received since
     the data was last cleared to a ``.blip`` file with the names of their
     connections, also those of an opened recording. On the web, the file is downloaded.
   * **Clear data** removes all received data but the latest units and
     actions of the targets, which they may send only when connecting or when
     they change. The recording starts again with them, so a saved recording
     has them too.
   * **Open layout** and **Save layout** load and save the layout of the
     views, with all their settings, as a JSON file. **Reset layout** returns
     to a plot and a log view.

   Recording and layout files can also be dropped onto the window.

View
   Zooms the whole UI in and out, also with :kbd:`Ctrl` and :kbd:`+`,
   :kbd:`-` and :kbd:`0` natively. In the browser, these keys zoom the page.
   The zoom is kept across restarts.

Help
   The version of blip and a link to this documentation.

The theme button on the right of the menu switches between the light, dark
and system theme.

Views
-----

Plot
   Time-series plot of the ``Value`` traces checked in the left panel while
   the plot is selected. Select a plot by clicking it or its row in the layout
   panel. Hovering a plot shows the values of its traces at the pointer.
   Numbers are drawn as lines and bools as steps between 0 and 1, both with
   gaps where a ``null`` was received. Strings cannot be plotted, show them
   in a states or table view.

Logs
   Table of received ``Log`` records with timestamp, level, component and
   message. Records outside the shown time range are greyed out.

Actions
   A form of the actions checked in the left panel, to carry them out on
   their outposts, e.g. set a motor setpoint or a log level. See `Actions`_.

Actions
-------

An actions view lists the actions checked in the left panel while it is
selected, none when it is added, an action per row: its name (hover it for the description, the path
and the outpost it is sent to), its input, the current value of its readback
trace if it has one, and what became of its latest invocation.

* An action without parameters is a **Run** button.
* An action with one parameter is sent as soon as its input changes: a
  checkbox for a bool, buttons or a dropdown for a choice, a slider for a
  number with a minimum and a maximum, sent while it is dragged (at most ten
  times a second, and the final value on release), a number field otherwise,
  sent when dragged like a slider or once a typed number is entered with
  :kbd:`Enter` or by leaving the field, and a text field for a string, sent
  with :kbd:`Enter`. Clicking a field to type in it sends nothing. Integers
  are typed in a text field and kept exact, also beyond 2\ :sup:`53`.
* An action with several parameters lists them below its row and is sent with
  its **Send** button.
* An action that asks for confirmation is sent after confirming it in a
  dialog.

The inputs start from the readback, the default of the parameter or its
lowest value. Checkboxes and choices with a readback follow it, so they show
the state of the target. After a change they keep the new value until a
readback sample arrives that is newer than the target's answer. blip only sends an action when the user does: never
on its own, at startup or when it connects again.

The last column shows a spinner while the target has not answered, a check
mark for a moment once it carried the action out, a warning if it rejected
the action (hover it for the reason) and a clock if it did not answer within
five seconds. Each invocation also shows as a log record of the component
``actions`` in the log views, rejected ones as warnings.

An action is disabled while the connection of its outpost is not connected,
e.g. while a recording is open, and while its target cannot carry it out.
Checked actions that no outpost announces, or that the chosen outpost does not
announce, are listed greyed out; the left panel lists them too, to uncheck
them. The properties of an actions view set:

* **Outpost**: the outpost all its actions are sent to, among those that
  announced actions. **First announcing**, the default, sends each action to
  the first outpost announcing it whose connection is open, else to the first
  announcing it, so the actions keep working when connecting to an outpost of
  another name. A chosen outpost that announces no actions any more is listed
  greyed out. The answers of an action, and whether it is connected, are
  those of the outpost it is sent to.
* **Readbacks**: for each action whether it has the readback its outpost
  announces, the trace showing the state the action sets, which the readback
  column shows and checkboxes and choices follow. The checkbox is disabled if
  the outpost announces none.
* **Number inputs** switches numbers with a minimum and a maximum between a
  slider and a field.

Properties
----------

The properties panel shows the settings of the selected view or group in
sections that can be collapsed. Changes apply at once and are saved with the
layout.

* A group has its name and how it arranges its children.
* A view has its name and type, the sections of its type, e.g. the y axis,
  lines and legend of a plot, and the **Number format** of its traces: how
  their numbers are written in the view (decimal, hex, binary or scientific),
  next to their unit as sent by the outpost. Hex and binary apply to whole
  numbers only, strings and bools have no format.
* An XY view plots the traces selected in the left panel against one of them.
  Its **Axes** set which trace is on the x axis, the others are on the y
  axis.
* Number fields left empty show *auto*: the value is fitted to the data.

Log views keep their search and filters in the view.

Layout
------

The layout panel shows the views as a tree of groups. The groups at the top
level are tabs. A group shows its children as tabs, stacked vertically or side
by side.

* Add a view, a tab or a group with the **+** button. A view or group is
  added next to the selected view or into the selected group. A new group
  stays while it is empty, to add views to it.
* Drag a row onto the top or bottom edge of another row to move it before or
  after that row, onto a group to move it into the group, or onto a view to
  put both into a new group. A new group stacks its children across the
  direction of its parent group.
* Drag a view by its title in the center, or a view or group shown as a tab by
  its tab, onto the side of another view. A line in the gap shows a move next
  to that view, where its group stacks its children across that side. A
  highlighted half shows a new group of both, stacked across that side; a
  group of just the two turns instead. Dropping onto a tab moves before or
  after it, and onto an empty group into it. Hovering a tab while dragging
  opens it. Views can also be dragged between the center and the layout panel.
* Right-click a row to rename or delete it, or to change the type of a group.
  Double-clicking a row also renames it, as does clicking the title of a view
  or the open tab in the center. Enter keeps the new name, Escape drops it.

Time range
----------

All views show the same time range, set in the bottom panel:

* **Record** keeps the values and log records received from the connections.
  While it is off they are dropped, as if the connections were closed, but
  the connections stay open: the targets' units and actions are still kept and
  actions can still be invoked. A recording starts only while a connection is
  open. Starting it clears the data as **File > Clear data** does and follows
  the newest data live. Stopping it keeps the data, also when the connections
  close. While recording, the connections cannot be changed but connected
  again. blip starts
  without recording, as does opening a recording file.
* **Pause** stops the range where it is, while recording goes on, to inspect
  the data. **Play** moves the range on from there at the pace of the
  timestamps, along with the time cursor, and returns to live mode once it
  reaches the newest data.
* **Live** follows the newest data, playing.
* **Window** sets the length of the range.
* The timeline spans all collected data, its thumb shows the range. Drag the
  thumb to move the range, click the timeline to center the range there, or
  scroll over it to change the window. Moving the range pauses, moving it to
  the end of the data returns to live mode.

The time cursor is a vertical line with a handle on top, shown in all plots
over time and on the timeline. The table, the heatmap at the cursor and the
marker of the XY view show the values at its time. It stays at the end of the
range until it is set: click a plot to put it there, or drag it in a plot or
on the timeline. Setting it pauses, and it stays set until **Live** is
pressed, which returns the cursor to the end of the range.

The time axes show the timestamps as local time of day, or as time since boot
if they are too small to be a date (before 2001).

Dragging a plot moves the range, and scrolling the mouse wheel over a plot zooms
it, which changes the window. The y axes fit the data in the range, unless
their min or max is set in the properties.

The layout, the time window and the connection addresses are persisted and restored
on the next start.

.. todo::

   Add screenshots of the plot and log views.
