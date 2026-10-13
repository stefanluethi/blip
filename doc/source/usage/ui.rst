Using the UI
============

Connecting
----------

#. Enter the WebSocket address of the bridge or target in the settings panel,
   e.g. ``ws://127.0.0.1:3000/ws/bridge``.
#. Press **Connect**. The button changes to **Disconnect** while connected.

Window
------

Top panel
   WebSocket address, **Connect** and **Clear Data**, which removes all
   received data.

Left panel
   The **Streams** of the selected plot, as a tree that branches at each
   ``/`` of the path, and the **Layout** of the views below.

Center
   The views, laid out as edited in the layout panel.

Bottom panel
   The time range shown by all views.

Views
-----

Plot
   Time-series plot of the ``Value`` streams checked in the left panel while
   the plot is selected. Select a plot by clicking it or its row in the layout
   panel. Hovering a plot shows the values of its streams at the pointer.

Logs
   Table of received ``Log`` records with timestamp, level, component and
   message. Records outside the shown time range are greyed out.

Layout
------

The layout panel shows the views as a tree of groups. The groups at the top
level are tabs. A group shows its children as tabs, stacked vertically or side
by side.

* Add a view or a tab with the **+** button. A view is added next to the
  selected view or into the selected group.
* Drag a row onto the top or bottom edge of another row to move it before or
  after that row, onto a group to move it into the group, or onto a view to
  put both into a new group. A new group stacks its children across the
  direction of its parent group.
* Right-click a row to rename or delete it, or to change the type of a group.
  Double-clicking a row also renames it.

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

Dragging a plot moves the range, and zooming a plot with :kbd:`Ctrl` and the
mouse wheel changes the window. The y axes fit the data in the range.

The layout, the time window and the target address are persisted and restored
on the next start.

.. todo::

   Add screenshots of the plot and log views.
