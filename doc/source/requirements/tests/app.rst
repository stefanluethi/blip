Application
===========

Tests of the requirements in :doc:`/requirements/system/app`.

.. test:: Keyboard shortcuts act and wait
   :id: TEST_BLIP_SHORTCUTS
   :status: implemented
   :component: blip
   :verifies: REQ_APP_SHORTCUTS

   ``blip/src/app/tests.rs`` -- ``space_plays_and_pauses``;
   ``escape_releases_the_cursor_once_the_connections_are_closed`` checks that
   Escape releases the cursor only when no field, menu, dialog or list of
   connections took it; ``delete_deletes_the_selected_view_until_undone_from_its_toast``;
   ``no_shortcut_fires_while_a_text_field_has_the_focus``;
   ``delete_waits_while_a_menu_is_open``; ``shortcuts_wait_while_a_question_is_open``;
   ``the_record_key_asks_like_the_button`` (R); ``saving_nothing_tells_so`` (Ctrl+S).
   ``kit/src/widgets/view/trace_selection/tests.rs::ctrl_f_focuses_the_search``
   (Ctrl+F). ``kit/src/workbench/layout/tests.rs::renaming_the_selected_view_opens_its_tab``
   (F2). The keys L and Ctrl+O and the list under Help are checked by hand.

.. test:: Notices have levels and buttons
   :id: TEST_BLIP_NOTICES
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_APP_NOTICES, REQ_LIB_NOTICES

   ``kit/src/workbench/toasts/tests.rs`` -- ``toasts_expire_after_they_are_shown``
   and ``errors_stay_until_they_are_closed`` check that neutral notices and
   warnings expire and errors do not; ``a_repeated_toast_is_counted_and_shown_for_longer``
   that a repeat is counted; ``notices_of_the_store_keep_their_level``;
   ``a_toast_is_shown_until_it_is_closed``; ``the_action_of_a_toast_is_given_back_once_clicked``
   that the button of a notice closes it and is read once;
   ``a_toast_is_dismissed_by_the_value_of_its_action``;
   ``the_close_button_is_as_high_as_the_controls``.
   ``kit/src/workbench/files/tests.rs::a_save_label_has_an_ellipsis_only_where_a_dialog_follows``
   -- the label of a Save item.

.. test:: Controls say what they do
   :id: TEST_BLIP_APP_LABELS
   :status: implemented
   :component: blip
   :verifies: REQ_APP_LABELS

   ``blip/src/app/tests.rs::presets_are_not_switched_while_recording`` -- the
   button of the Presets menu is found by "Presets: Preset 1". The labels of
   the panel and theme buttons, the entries of the View and Help menus and the
   new tab of the documentation are checked by hand.
