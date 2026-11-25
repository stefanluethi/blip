Mouse and keyboard
==================

A summary of the controls described in the other pages.

Keyboard shortcuts
------------------

**Help › Keyboard shortcuts** lists them in blip.

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Key
     - Does
   * - :kbd:`R`
     - start or stop recording
   * - :kbd:`Space`
     - play or pause
   * - :kbd:`L`
     - go live
   * - :kbd:`Escape`
     - release the time cursor, back to the end of the window
   * - :kbd:`Ctrl+F`
     - search the traces: shows the traces panel and focuses its search field
   * - :kbd:`F2`
     - rename the selected view
   * - :kbd:`Delete`
     - delete the selected view, see :doc:`/blip/layout`
   * - :kbd:`Ctrl+O`
     - open a recording
   * - :kbd:`Ctrl+S`
     - save the recording

None of them works while a text field has the focus or a dialog is open.
:kbd:`Space` and :kbd:`Delete` wait while a menu is open, as menus take the
keys. :kbd:`Escape` first cancels a field, closes a menu, a dialog or the
connections, and only when none of them is open releases the time cursor.
:kbd:`F2` and :kbd:`Delete` act on the selected view if it is in the tab that
is shown.

In the traces tree, the arrow keys move between rows, :kbd:`Left` and
:kbd:`Right` close and open a folder and :kbd:`Space` checks the row.

Anywhere
--------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Input
     - Does
   * - :kbd:`Ctrl+O`
     - open a recording
   * - :kbd:`Ctrl+S`
     - save the recording
   * - :kbd:`Ctrl` + :kbd:`+` / :kbd:`-` / :kbd:`0`
     - zoom the UI (native app; the browser zooms the page)
   * - drop a file on the window
     - open a recording or import presets; *Drop to open* shows while the file
       is over the window

Plots and states over time
--------------------------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Input
     - Does
   * - hover
     - values at the pointer
   * - click
     - put the time cursor there (pauses)
   * - drag the cursor line
     - move the time cursor
   * - drag sideways
     - move the time range (pauses)
   * - drag up or down
     - pan the y axis (plots)
   * - mouse wheel
     - zoom the time range, i.e. change the window
   * - :kbd:`Alt` + mouse wheel
     - zoom the y axis (plots)
   * - double-click
     - fit the y axis to the data again (plots); the time range and the cursor
       stay

A drag waits until it has moved 6 points to tell whether it goes sideways or
up and down.

The XY view pans with dragging and zooms with the wheel along both axes. A
distribution pans with dragging and zooms with the wheel along its value axis,
and double-clicking fits it again; it leaves the time range and the cursor
alone. Hovering a bin or a box shows its counts or statistics.

Timeline
--------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Input
     - Does
   * - drag the thumb
     - move the range
   * - drag an edge of the thumb
     - move the start or end of the range
   * - click the track
     - center the range there
   * - press or drag on the ruler
     - set the time cursor and move the window with it
   * - mouse wheel
     - zoom the timeline
   * - :kbd:`Shift` + mouse wheel, or sideways
     - move the timeline
   * - click the log records of a bar
     - put the cursor at the most severe one

Center
------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Input
     - Does
   * - click a view
     - select it
   * - click a title
     - rename (:kbd:`Enter` keeps, :kbd:`Escape` drops)
   * - click a tab
     - select it
   * - double-click a tab
     - rename it
   * - **✕** on a hovered tab
     - delete the tab and its views
   * - **⋯** on a title, right-click a title or tab
     - add a view right or below, rename, delete
   * - **View › Add view**
     - add a view after the selected one
   * - click a view type in an empty tab
     - add a view of that type
   * - **+** after the tabs
     - add a tab
   * - drag a view title or a tab
     - move it, see :doc:`/blip/layout`
   * - **View › Undo delete**, **Undo** on the toast
     - bring back what was deleted last

Traces tree
-----------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Input
     - Does
   * - click a checkbox
     - show or hide the trace in the selected view
   * - click a caret
     - open or close the folder
   * - :kbd:`Alt` + click a caret
     - open or close everything below it
   * - **⋯** beside the search
     - expand all or collapse all
   * - hover a folder
     - its path, and whether a check on it also shows traces added later

Connections
-----------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Input
     - Does
   * - click the connection chip in the menu bar
     - open or close the connections
   * - click elsewhere in the window, :kbd:`Escape`
     - close the connections
   * - click a connection
     - connect or disconnect
   * - right-click a connection
     - connect, disconnect, rename, edit the address, delete
   * - hover a red dot
     - why the connection failed
