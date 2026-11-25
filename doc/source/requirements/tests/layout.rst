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
