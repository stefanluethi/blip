Layout
======

Views, tabs and groups of the layout, and their properties.

.. req:: Undo a delete of the layout
   :id: REQ_LAY_UNDO
   :status: implemented
   :component: blip, blip-kit
   :satisfies: STK_CONFIDENCE
   :rationale: A view is set up with care, and a click or a key deletes it.

   Deleting a view, a tab or a group, and View › Clear layout, shall tell what
   was deleted in a notice such as "Deleted Plot 1" with an Undo button, and
   enable View › Undo delete. Undoing shall bring back what was deleted, with
   the links of views to each other, and select it, and shall keep the changes
   made to the other views since. A delete shall be undone only until the layout
   changes in another way, and only the last delete shall offer to be undone.

.. req:: Tabs of the layout
   :id: REQ_LAY_TABS
   :status: implemented
   :component: blip, blip-kit
   :satisfies: STK_LIVE_OBSERVE

   A click on a tab shall select it and a double click shall rename it. A tab
   hovered shall show a ✕ that deletes it with its views, and its tooltip shall
   say how to rename and to move it. A view shown as a tab shall have the menu of
   a view. This shall hold for the tabs of a group at any depth of the layout,
   not only for those at the top. Every tab bar shall have a **+** after its
   last tab that adds a tab at the end of that group and opens it, and the new
   tab shall be empty and show the view types.

.. req:: Properties of groups and views
   :id: REQ_LAY_PROPS
   :status: implemented
   :component: blip, blip-kit
   :satisfies: STK_LIVE_OBSERVE

   The properties of a group shall offer Arrange as a choice of Tabs,
   Vertical and Horizontal. While the name of a view or group is blank in its field, the
   field shall show the name kept as a hint and the name shall come back when
   the field loses the focus. A long name shall be cut off before the kind of
   the view, which shall not be a button.

.. req:: Groups at every level
   :id: REQ_LAY_NEST
   :status: implemented
   :component: blip-kit
   :satisfies: STK_LIVE_OBSERVE
   :rationale: A layout is not always a row of tabs of views: one tab may need a plot over a group of tabs of logs and tables.

   blip shall let a group shown as tabs, Vertical or Horizontal hold views and
   groups of any kind at any depth. A group at any depth shall have the
   properties of :need:`REQ_LAY_PROPS`. A tab at any depth shall be renamed,
   deleted with undo (:need:`REQ_LAY_UNDO`) and moved by drag as a tab at the top
   is.

.. req:: Nested groups are kept
   :id: REQ_LAY_NEST_SAVED
   :status: implemented
   :component: blip-kit
   :satisfies: STK_LIVE_OBSERVE

   A layout with groups inside groups shall be kept in the active preset and in
   a presets file, and shall come back with the same groups and their kinds,
   names, order and open tabs, and the same views. A presets file written
   before groups were allowed at every level shall load as it did.

.. req:: Drop at the side of a view
   :id: REQ_LAY_DROP_SIDE
   :status: implemented
   :component: blip-kit
   :satisfies: STK_LIVE_OBSERVE

   While a view, or a group shown as a tab, is dragged over a view, blip shall
   highlight the half of that view at the side nearest to the pointer, outside
   the middle of the view (:need:`REQ_LAY_DROP_CENTER`), and dropping shall put
   the dragged one at that side. Where the group of the view already arranges
   its children in the direction of that side (Vertical above and below,
   Horizontal left and right), the dragged one shall go next to the view. In
   any other case, the view and the dragged one shall form a new group in the
   direction of that side, in the place of the view, and a group of just the two
   shall turn instead. This shall hold for a view at any depth.

.. req:: Drop at the middle of a view
   :id: REQ_LAY_DROP_CENTER
   :status: implemented
   :component: blip-kit
   :satisfies: STK_LIVE_OBSERVE

   While a view, or a group shown as a tab, is dragged over the middle of a view,
   blip shall highlight that view as a whole, and dropping shall make a new group
   shown as tabs of the view and the dragged one, in the place of the view, with
   the dragged one after the view and open. Where the view and the dragged one
   are the only two of a Vertical or Horizontal group, that group shall turn
   into one shown as tabs instead. The middle shall be the view inset by a
   quarter of its width and of its height, and at least 40 by 40 pixels. This
   shall hold for a view at any depth.

.. req:: Drop on a tab row
   :id: REQ_LAY_DROP_TABS
   :status: implemented
   :component: blip-kit
   :satisfies: STK_LIVE_OBSERVE

   While a view, or a group shown as a tab, is dragged over the row of the tabs
   of a group shown as tabs, blip shall show where it goes, and dropping shall
   add it as another tab of that group: before or after a tab where dropped on
   that tab, and as the last tab where dropped on the empty part of the row.
   This shall hold for the tab row of a group at any depth, the top one
   included. Hovering a tab while dragging shall open it, and dropping onto an
   empty tab shall put the dragged one into it.

.. req:: Add an empty group from a menu
   :id: REQ_LAY_ADD_MENU
   :status: implemented
   :component: blip-kit
   :satisfies: STK_LIVE_OBSERVE

   The menu of a view, and the menu of a tab below the top level, shall have
   Add right and Add below, each listing the view types and then **Empty group**.
   A view type shall add a view of that type as if dropped at that side
   (:need:`REQ_LAY_DROP_SIDE`). Empty group shall add, at that side, a group
   shown as tabs with one tab, named "Tab" and a number, that is empty and shows
   the view types. For a tab, the side shall be that of the whole group of the
   tab. The menu of a tab at the top level shall not have these entries.

.. req:: Empty groups show the view types
   :id: REQ_LAY_EMPTY
   :status: implemented
   :component: blip-kit
   :satisfies: STK_LIVE_OBSERVE

   A tab with no view, at any depth, shall show the view types, and a click on a
   type shall add a view of that type into the tab. Such a tab shall stay until
   it is filled or deleted: it shall also stay when its last view is deleted.
   It shall be renamed and deleted as any tab.

.. req:: View types in a small group
   :id: REQ_LAY_PICKER_SMALL
   :status: implemented
   :component: blip-kit
   :satisfies: STK_LIVE_OBSERVE
   :rationale: A group at the third level is often too small for the list of the types.

   Where an empty tab is too small for the list of the view types, blip shall
   show them as a row of icon buttons, each with its type name on hover, and
   where not even that fits, as one menu button listing them. Every view type
   shall be reachable in an empty tab of any size.

.. req:: Empty groups are removed
   :id: REQ_LAY_PRUNE
   :status: implemented
   :component: blip-kit
   :satisfies: STK_LIVE_OBSERVE, STK_CONFIDENCE

   A Vertical or Horizontal group that loses its last view or group, by a
   delete or by a move, shall be removed, and a delete shall be undone by
   :need:`REQ_LAY_UNDO`. A tab that loses its last view by a move shall be
   removed, and one that loses it by a delete shall stay, empty
   (:need:`REQ_LAY_EMPTY`). A group shown as tabs that has no tab left below the
   top level shall be removed. This shall hold at any depth.

.. req:: Tabbed cards below the top level
   :id: REQ_LAY_CARD
   :status: implemented
   :component: blip, blip-kit
   :satisfies: STK_LIVE_OBSERVE

   blip shall show the tabs of a group below the top level in the title row of
   one card, joined to the open view or group below them, with the **+** after the
   last tab. The tabs at the top level shall stay on the canvas. A view shown
   as a tab shall keep its legend in the corner of its plot, not in the tab row,
   and the titles of tabs shall not be cut off. The look shall be the same in
   the light and the dark theme.

.. req:: The empty part of a nested tab row
   :id: REQ_LAY_NEST_ROW
   :status: implemented
   :component: blip-kit
   :satisfies: STK_LIVE_OBSERVE
   :priority: low

   The empty part of the tab row of a group below the top level shall not be
   dragged as the group and shall have no menu. It shall take a drop as
   :need:`REQ_LAY_DROP_TABS` says.
