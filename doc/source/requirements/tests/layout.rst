Layout
======

Tests of the requirements in :doc:`/requirements/system/layout`.

.. test:: A delete of the layout is undone
   :id: TEST_BLIP_LAYOUT_UNDO
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_LAY_UNDO

   ``kit/src/workbench/layout/tests.rs`` --
   ``a_deleted_view_comes_back_by_undo_with_its_links``,
   ``a_deleted_tab_comes_back_by_undo_and_opens`` and
   ``clearing_the_layout_can_be_undone`` check that a view, a tab and the whole
   layout come back; ``undo_brings_back_the_layout_before_the_last_delete``;
   ``undoing_a_delete_keeps_the_changes_of_the_other_views``;
   ``changing_the_layout_after_a_delete_forgets_the_undo``;
   ``each_delete_has_its_own_undo_number``;
   ``clearing_an_empty_layout_keeps_the_undo_of_the_last_delete``.
   ``blip/src/app/tests.rs`` --
   ``delete_deletes_the_selected_view_until_undone_from_its_toast`` checks the
   notice "Deleted <name>" with Undo and that its button undoes;
   ``only_the_last_delete_offers_to_be_undone`` that the notice of an earlier
   delete goes. ``kit/src/workbench/toasts/tests.rs`` --
   ``the_action_of_a_toast_is_given_back_once_clicked`` and
   ``a_toast_is_dismissed_by_the_value_of_its_action``.

.. test:: Tabs are selected, renamed and deleted
   :id: TEST_BLIP_TABS
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_LAY_TABS

   ``kit/src/workbench/layout/tests.rs`` --
   ``double_clicking_a_tab_renames_it_and_a_click_only_selects_it``,
   ``the_cross_on_a_hovered_tab_deletes_it``,
   ``a_tab_tells_how_to_rename_and_move_it`` and
   ``a_view_shown_as_a_tab_has_the_menu_of_a_view``.

.. test:: Properties of groups and views
   :id: TEST_BLIP_LAYOUT_PROPS
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_LAY_PROPS

   ``kit/src/workbench/layout/properties/tests.rs`` --
   ``groups_are_arranged_by_a_choice_of_three`` checks the choice of Horizontal
   and Tabs; ``a_blank_name_shows_the_name_kept_and_comes_back_on_leaving_the_field``;
   ``a_long_name_is_cut_off_before_the_kind``.

.. test:: Tabs of nested groups
   :id: TEST_BLIP_TABS_NESTED
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LAY_TABS

   ``kit/src/workbench/layout/tests.rs`` --
   ``the_plus_adds_an_empty_tab_at_the_end_of_its_group`` checks on the tree at
   depth 2 and 3 that the new tab is empty, vertical, named "Tab n", last, open
   and selected; ``the_plus_of_a_nested_tab_bar_adds_a_tab_and_opens_it`` that the
   **+** of a nested tab bar and the top-level **+** each add a tab;
   ``nested_tabs_are_opened_selected_and_renamed_by_clicks`` that a click on a
   nested tab opens it and keeps the selection, a second selects it and a double
   click renames it, at depth 3 and 4; ``the_cross_on_a_hovered_nested_tab_deletes_it``
   (with Undo); ``a_nested_tab_tells_how_to_rename_and_move_it``;
   ``a_view_shown_as_a_nested_tab_has_the_menu_of_a_view``;
   ``the_plus_of_a_nested_tab_bar_opens_a_tab_that_shows_the_view_types`` that
   after a click on the **+** of a nested tab bar the Heatmap icon of the view
   types lies in the card of the new tab and a click on it adds a view.

.. test:: Groups at every level
   :id: TEST_BLIP_NEST
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LAY_NEST, REQ_LAY_PROPS

   ``kit/src/workbench/layout/tests.rs`` unit tests on a layout three groups deep
   (``nested()``) and ``kit/src/workbench/layout/properties/tests.rs``: groups of
   each kind hold views and groups at depth 2 to 4 and keep their children when
   their kind is changed; a nested tab is renamed and moved by the tree and by
   dragging with ``egui_kittest``; deleting a nested tab removes its views and the
   links to them, and Undo brings back the tab, its views, the links and the
   selection; the properties panel changes Arrange and the name of a group at
   depth 2 and 3.

.. test:: Nested groups survive saving and loading
   :id: TEST_BLIP_NEST_SAVED
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LAY_NEST_SAVED

   ``kit/src/workbench/layout/tests.rs`` unit tests: ``nested()``, with open tabs
   other than the first and an empty tab, is saved to JSON and loaded again, and
   the names, order, kinds and open tabs are the same and saving it again gives
   the same JSON; a presets file with nested groups from before this change loads
   with its kinds, names, shares, open tab and empty group.

.. test:: Drops at the side and the middle of a view
   :id: TEST_BLIP_DROP_VIEW
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LAY_DROP_SIDE, REQ_LAY_DROP_CENTER

   ``kit/src/workbench/layout/tests.rs`` unit tests of the layout tree on
   ``nested()`` and ``default_layout()``, the tree part: dropping at a side of a
   view at depth 3 and 4 goes next to it along its group, makes a new group across
   it, or turns a group of just the two; dropping at the middle of a view makes a
   group shown as tabs of the view and the dragged one in its place, the dragged
   one last and open, and turns a group of just the two into tabs; a group is not
   dropped into itself at any depth. The pointer part:
   ``kit/src/workbench/layout/drag/tests.rs`` --
   ``the_middle_of_a_view_is_inset_by_a_quarter_and_at_least_40_px``,
   ``a_card_takes_the_middle_as_a_whole_and_the_rest_by_side`` and
   ``the_marker_is_the_half_at_a_side_and_the_whole_view_in_the_middle`` check the
   middle zone, the drop it gives and the marked area; ``kit/src/workbench/layout/tests.rs``
   -- ``dragging_a_view_to_the_middle_of_another_stacks_them_as_tabs`` drags with
   ``egui_kittest`` to the middle of a view and checks the group of tabs.

.. test:: Drops on a tab row
   :id: TEST_BLIP_DROP_TABS
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LAY_DROP_TABS, REQ_LAY_NEST_ROW

   ``kit/src/workbench/layout/tests.rs`` unit tests of the layout tree on
   ``nested()``, the tree part: dropping before or after a tab adds the dragged
   view or group there as a tab at depth 1 to 3; dropping into a group shown as
   tabs, as on the empty part of its row, adds it as the last tab, opened and
   selected; dropping onto an empty tab puts the dragged one into it. The pointer
   part: ``kit/src/workbench/layout/drag/tests.rs`` --
   ``the_empty_part_of_a_tab_row_takes_the_dragged_one_as_last_tab`` checks the target
   of the empty part of a row, its marked area and line, and that a tab button comes
   first; ``kit/src/workbench/layout/tests.rs`` with ``egui_kittest`` --
   ``dropping_on_the_empty_part_of_a_nested_tab_row_adds_the_last_tab``,
   ``hovering_a_nested_tab_while_dragging_opens_it_and_an_empty_tab_takes_the_drop``
   and ``the_empty_part_of_a_nested_tab_row_drags_nothing_and_has_no_menu`` (nothing
   moves and a right click shows no Rename or Delete).

.. test:: Add right and Add below
   :id: TEST_BLIP_ADD_MENU
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LAY_ADD_MENU

   ``kit/src/workbench/layout/tests.rs`` --
   ``the_menus_list_the_view_types_then_empty_group_and_top_tabs_have_none`` (with
   ``egui_kittest``) checks that the menus of a view and of a tab below the top level
   list the view types and then Empty group, that Empty group adds the group, and
   that the menu of a top-level tab has no Add right or Add below;
   ``empty_group_from_the_menu_adds_a_tab_group_with_one_empty_tab`` checks on the
   tree that Empty group at a side of a view, in a column, a row and a group of
   tabs, adds a group of tabs "Group n" with one empty tab "Tab n", selected and
   open, and nothing at the top level;
   ``add_right_and_below_on_a_nested_tab_go_at_the_side_of_its_whole_group`` checks
   that a view type or Empty group beside a nested tab goes at the side of its whole
   group, keeping the tab.

.. test:: Empty tabs show the view types
   :id: TEST_BLIP_EMPTY_GROUP
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LAY_EMPTY, REQ_LAY_PICKER_SMALL

   ``kit/src/workbench/layout/picker/tests.rs``
   ``the_view_types_shrink_to_icons_then_to_a_menu`` checks the form of the
   picker for 8 types at ten sizes (list, icons, menu) at both sides of each
   limit. ``kit/src/workbench/layout/tests.rs`` with ``egui_kittest``:
   ``a_view_type_clicked_in_an_empty_tab_is_added_to_it`` (the list);
   ``a_small_empty_tab_shows_the_view_types_as_icons`` (no type name shown, the
   name on hover of the icon, a click adds the view and selects it);
   ``a_tiny_empty_tab_shows_the_view_types_in_a_menu`` (no icon of a type shown,
   the button "Add a view" opens a menu and a type chosen adds the view);
   ``an_empty_nested_tab_shows_the_view_types_and_takes_a_view`` (an empty tab at
   depth 3 and a new one at depth 2: the icons lie inside the rect of the tab
   and inside the card of its group, below the tab row, and a click adds a
   view); ``an_empty_nested_tab_stays_until_filled_or_deleted`` on the tree (the
   tab stays when its last view is deleted, is renamed, deleted, and comes back
   empty by Undo).

.. test:: Empty groups are removed
   :id: TEST_BLIP_PRUNE
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LAY_PRUNE

   ``kit/src/workbench/layout/tests.rs`` unit tests on ``nested()``: a Vertical or
   Horizontal group left empty by a delete or a move is removed with the groups
   above it that it leaves empty, and Undo of the delete brings them back; a tab
   left empty by a delete stays and by a move is removed; a group shown as tabs
   below the top level whose last tab is deleted or moved is removed and Undo of
   the delete brings it back; the top level stays.

.. test:: Nested tab groups are tabbed cards
   :id: TEST_BLIP_NEST_SHOT
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_LAY_CARD

   ``blip/src/app/screenshots.rs::screenshots_layouts`` renders
   ``layout-nested-tabs``, a plot beside a tab group of two views, from the demo
   recording in the light and the dark theme, for the layout guide and to check
   by eye (design review) that the tabs sit in the title row of one card joined
   to the open view, the **+** follows the last tab, the legend is in the corner
   of the plot and the top tabs lie on the canvas; it asserts nothing.
   ``kit/src/workbench/layout/tests.rs``
   ``nested_tabs_lie_in_the_title_row_of_one_card`` with ``egui_kittest``: the tab
   labels of the groups at depth 2 and 3 lie in the top ``TAB_BAR_HEIGHT`` of the
   rect of their group, the open view starts ``GAP`` below it with the width of the
   group, and the label of a view of the tab at the top level lies at least
   ``GAP`` below the top tab label.
   ``a_tabbed_card_starts_as_high_as_the_card_of_a_view_beside_it`` and
   ``a_top_tab_of_only_a_tab_group_leaves_the_gap_above_its_card``: the rect of a
   tab group has the top of the view beside it and lies ``GAP`` under the top
   tab row, also as the only child of a top tab, and its tab row starts at that
   top. ``a_tab_group_in_the_open_tab_of_a_tabbed_card_lies_a_gap_inside_its_card``:
   the rect and the tab row of a tab group in the open tab of another lie inside
   the rect of the outer one, at least ``GAP`` under its row.
   ``the_remaining_open_tab_of_a_card_is_marked_open_in_the_frame_the_open_tab_leaves``:
   the first frame after the open tab of a group is dragged out shows the
   underline of the tab that opens in its place.
