Layout and properties
=====================

The views are arranged in **tabs** in the center. A tab holds its views in
**groups**, stacked vertically or side by side, or shown as tabs again. blip
starts with one tab holding a plot and a log view; **File › Reset layout**
returns to that.

Adding views and tabs
---------------------

Click **+** after the tabs to add a tab. An empty tab shows the view types
(see :doc:`views`); click one to add a view of that type.

To add a view next to another, right-click the title of that view (or its tab,
if it is shown as a tab) and choose **Add right** or **Add below** and the view
type. The new view goes there as if it was dragged to that side, see below.

Selecting
---------

Click a view to select it. The traces panel then chooses what the view shows
and the properties panel shows its settings. Switching tabs keeps the selected
view; clicking the open tab selects it, and the properties panel shows the
settings of its group.

Arranging
---------

Drag a view by its title, or a view or group shown as a tab by its tab, onto
the side of another view:

* A **line in the gap** shows a move next to that view, where its group stacks
  its children across that side.
* A **highlighted half** shows a new group of both, stacked across that side; a
  group of just the two turns instead.
* Dropping onto a tab moves before or after it, and onto an empty tab into
  it. Hovering a tab while dragging opens it.

Moving the last view out of a tab removes the tab.

Renaming and deleting
---------------------

Click the title of a view, or the open tab, to rename it, or edit the **Name**
in the properties. :kbd:`Enter` keeps the new name, :kbd:`Escape` drops it.

Right-click the title of a view, or a tab, to **Rename** or **Delete** it.
Deleting a tab deletes the views in it. Deleting the last view of a tab keeps
the tab, to add other views to it.

Properties
----------

The **Properties** panel shows the settings of the selected view or group in
sections that can be collapsed. Changes apply at once and are saved with the
layout.

* A **group** has its **Name** and **Arrange**: whether it shows its children
  as *Tabs*, *Vertical* or *Horizontal*.
* A **view** has its **Name**, the sections of its type, and the **Number
  format** of its traces, see :doc:`views`.

Log views keep their search and filters in the view itself.

Saving layouts
--------------

blip keeps the layout across restarts. **File › Save layout** saves it, with
all settings of the views, to a JSON file, and **File › Open layout** opens
one, e.g. a layout per project or per target. See :doc:`files`.
