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
   a view.

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
