Layout and properties
=====================

The views are arranged as a **tree of groups**, shown in the **Layout** panel
at the bottom left and laid out in the center. The groups at the top level are
**tabs**. A group below shows its children as tabs, stacked vertically or side
by side. blip starts with one tab holding a plot and a log view; **File ›
Reset layout** returns to that.

Adding views and groups
-----------------------

Click **+** in the layout panel and choose a view type (see :doc:`views`), a
**Tab**, or a **Tabs**, **Vertical** or **Horizontal group**. A view or group is
added next to the selected view or into the selected group. A new group stays
while it is empty, to add views to it.

Selecting
---------

Click a view in the center, or a row in the layout panel, to select it. The
traces panel then chooses what the view shows and the properties panel shows
its settings. Selecting a group shows the settings of the group.

Arranging in the layout panel
-----------------------------

Drag a row

* onto the top or bottom edge of another row to move it before or after that
  row,
* onto a group to move it into the group,
* onto a view to put both into a new group. A new group stacks its children
  across the direction of its parent group.

Arranging in the center
-----------------------

Drag a view by its title, or a view or group shown as a tab by its tab, onto
the side of another view:

* A **line in the gap** shows a move next to that view, where its group stacks
  its children across that side.
* A **highlighted half** shows a new group of both, stacked across that side; a
  group of just the two turns instead.
* Dropping onto a tab moves before or after it, and onto an empty group into
  it. Hovering a tab while dragging opens it.

Views can also be dragged between the center and the layout panel.

Renaming and deleting
---------------------

Right-click a row to rename or delete it, or to change the type of a group.
Double-clicking a row also renames it, as does clicking the title of a view or
the open tab in the center, or editing the **Name** in the properties.
:kbd:`Enter` keeps the new name, :kbd:`Escape` drops it.

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
