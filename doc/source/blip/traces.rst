Traces, actions and logs
========================

The **Traces, actions and logs** panel at the top left lists everything the targets
announced or sent, as a tree, and chooses what the **selected view** shows.
Select a view by clicking it.

.. figure:: /screenshots/panel-traces.png
   :target: ../_images/panel-traces.png
   :class: screenshot
   :width: 300px
   :alt: The traces tree while a plot is selected: imu.acc is checked, strings and actions are disabled.

   Choosing the traces of a plot.

The tree
--------

* The tree branches at each ``/`` of a path. Each trace shows the icon of its
  type at the end of its row, and on hover its path, its type, e.g. ``float``,
  ``int[8×8]`` or ``string``, and the description the target sent, if any; each announced action shows a lightning, and
  the logs of a path a list.
* The fields of a record are separate traces joined to its path with ``.``,
  e.g. ``imu.acc``, listed below the record as ``.acc``.
* Arrays list their elements, e.g. ``imu.acc[0]``, if they have at most 16;
  larger arrays cannot be plotted element by element yet. Heatmaps show
  arrays of one or two dimensions as a whole, and an XY view plots a
  one-dimensional array over the indices of its elements.
* What the selected view cannot show is disabled, with the reason on hover:
  e.g. strings in a plot, floats in a states view, traces in an actions view,
  actions in any other view or logs in any but a log view. With no view
  selected, the panel says *Select a view to choose its traces, actions or
  logs* and only the checkboxes are disabled: folders still open.
* **Alt+click** on a caret opens or closes the whole subtree. The arrow keys
  move between the rows, :kbd:`Left` and :kbd:`Right` close and open a folder
  and :kbd:`Space` checks the row. Long names are cut with *…*; hover for the
  full path.
* The **⋯** menu beside the search has **Expand all** and **Collapse all**.

Checking traces
---------------

* The checkboxes choose the traces a view shows, the actions of an actions
  view or the logs of a log view. A new log view shows all logs. The checked
  logs of paths without records yet, e.g. of an opened layout, are listed too,
  so they can be unchecked.
* In a plot or an XY view, the box of a checked trace takes the colour of its
  line, so the tree also tells the lines apart.
* **Checking a branch also checks what is added below it later**, e.g. a trace
  that arrives after it was checked. Unchecking anything below it ends that.
  A folder with this mark has a faint **+** at the end of its row, and its
  hover says *Also shows traces added here later*; the hover of an unchecked
  folder says *Check to also show traces added later*.
* A view that shows one trace, the heatmap, has **radio buttons** instead of
  checkboxes: choosing a trace replaces the one before. In an XY view, the
  hover of a whole array says that it replaces the other traces of the view.
* A table in the **Records** layout checks records as a whole instead of their
  fields, see :doc:`/blip/views/table`.

Searching and filtering
-----------------------

* The search field above the tree shows only the traces, actions and logs whose path
  contains each of its words, ignoring case. While searching, checking a
  branch checks only the matching traces, and adds nothing that arrives
  later.
* The filter button beside it shows only what the selected view can show,
  e.g. the numbers for a plot, the actions for an actions view or the logs for
  a log view. It stays on until it is clicked again, and is disabled while no
  view is selected, which shows all.
* The **checked only** button shows only what is checked for the selected
  view. When the search or a filter hides checked traces, a line under the
  field counts them, e.g. *2 checked traces are hidden by the search or the
  filter*. With nothing to show, the panel says why, e.g. *Nothing is checked
  for the selected view* or *Nothing checked matches the search*.
* :kbd:`Ctrl+F` shows the panel and puts the focus in the search field.

When a recording file is opened, and when a connection opens while no other is
open, the panel starts anew: the search is cleared, the tree is scrolled to
the top and its branches are open.

Actions a view shows that no outpost announces any more stay in the tree,
greyed out, so they can be unchecked.

Next steps
----------

* :doc:`/blip/views/index` -- what each type of view shows and its settings.
* :doc:`/blip/layout` -- add and arrange views.
