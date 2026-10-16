Using the UI
============

Connecting
----------

The **Connections** panel below the properties lists the bridges and targets
by their name and WebSocket address, starting with
``ws://127.0.0.1:3000/ws/bridge`` of a bridge started with its defaults, named
``bridge``. Several of them can be connected at once.

* Add an address with the **+** button, Enter keeps it, Escape drops it. The
  new connection is named after the last part of the path of the address,
  e.g. ``robby`` for ``ws://192.168.1.10/ws/robby``, numbered if another
  connection has the name.
* Click a connection to connect it, click it again to disconnect it. The
  connections are independent, the data received so far is kept; clear it
  with **File > Clear data**.
* Right-click a connection to connect or disconnect it, to rename it, to edit
  its address or to delete it. Rename it while it is disconnected.

The traces of a connection are listed below its name, e.g. ``cpu/load`` of
the connection ``robby`` as ``robby/cpu/load``, and the components of its logs
start with the name, e.g. ``robby/motor``. Views keep the paths with the name,
so a layout shows the same traces after reconnecting. A connection whose name
is empty adds nothing to the paths, as do the messages of a recording saved by
the bridge or by an older blip. The names of two connections differ, unless
they are empty.

Each target sends its own timestamps. Targets sending the time since boot
rather than the time of day are shown next to each other only as far as they
were started at the same time.

The dot before each connection shows its state: hollow when disconnected,
a spinner while connecting, green when connected and red when connecting
failed or the connection broke. Hover the red dot for the reason.

Window
------

Top panel
   The menu, see `Menu`_.

Left panel
   The **Traces** of the selected plot, as a tree that branches at each
   ``/`` of the path, with the type of each trace, and the **Layout** of the
   views below. Drag the border between them to resize them. The fields of a
   record are separate traces joined to its path with ``.``, e.g.
   ``imu.acc``, listed below the record as ``.acc``. Arrays list their
   elements, e.g. ``imu.acc[0]``, if they have at most 16; larger arrays
   cannot be plotted yet. The search field above the traces shows only those
   whose path contains each of its words, ignoring case.

Right panel
   The **Properties** of the selected view or group, see `Properties`_, and
   the **Connections** below, see `Connecting`_. Drag the border between them
   to resize them.

Center
   The views, laid out as edited in the layout panel.

Bottom panel
   The time range shown by all views.

Menu
----

File
   * **Open recording** (:kbd:`Ctrl+O`) disconnects all connections and
     shows the data of a
     recording file as if it had been received, with the time range fitted
     to all of it.
   * **Save recording** (:kbd:`Ctrl+S`) saves all messages received since
     the data was last cleared to a ``.blip`` file with the names of their
     connections, also those of an opened recording. On the web, the file is downloaded.
   * **Clear data** removes all received data.
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

Properties
----------

The properties panel shows the settings of the selected view or group in
sections that can be collapsed. Changes apply at once and are saved with the
layout.

* A group has its name and how it arranges its children.
* A view has its name and type, the sections of its type, e.g. the y axis,
  lines and legend of a plot, and the **Number format** of its traces: how
  their numbers are written in the view (decimal, hex, binary or scientific),
  next to their unit as sent by the bridge. Hex and binary apply to whole
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
* Right-click a row to rename or delete it, or to change the type of a group.
  Double-clicking a row also renames it, as does clicking the title of a view
  or the open tab in the center. Enter keeps the new name, Escape drops it.

Time range
----------

All views show the same time range, set in the bottom panel:

* **Live** follows the newest data. Clicking it while live stops at the
  current time.
* **Window** sets the length of the range.
* The timeline spans all collected data, its thumb shows the range. Drag the
  thumb to move the range, click the timeline to center the range there, or
  scroll over it to change the window. Moving the range to the end of the
  data returns to live mode.

The time cursor is a vertical line with a handle on top, shown in all plots
over time and on the timeline. The table, the heatmap at the cursor and the
marker of the XY view show the values at its time. It stays at the end of the
range until it is set: click a plot to put it there, or drag it in a plot or
on the timeline. Setting it stops live mode until **Live** is pressed, which
returns the cursor to the end of the range.

The time axes show the timestamps as local time of day, or as time since boot
if they are too small to be a date (before 2001).

Dragging a plot moves the range, and zooming a plot with :kbd:`Ctrl` and the
mouse wheel changes the window. The y axes fit the data in the range, unless
their min or max is set in the properties.

The layout, the time window and the connection addresses are persisted and restored
on the next start.

.. todo::

   Add screenshots of the plot and log views.
