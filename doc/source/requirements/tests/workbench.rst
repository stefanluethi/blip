Library: workbench and widgets
==============================

Tests of the requirements in :doc:`/requirements/system/workbench`.

.. test:: The session tells what is unsaved
   :id: TEST_LIB_UNSAVED
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LIB_UNSAVED

   ``kit/src/store/session/tests.rs`` --
   ``recorded_samples_are_unsaved_until_saved_cleared_or_replaced`` checks that
   dropped samples are not unsaved and recorded ones are until marked saved,
   cleared or replaced; ``the_first_source_to_open_keeps_an_unsaved_recording``;
   ``a_kept_recording_takes_no_logs_of_a_new_connection``;
   ``a_session_waits_for_record_while_connected_without_samples``.

.. test:: The layout is undone through its API
   :id: TEST_LIB_LAYOUT_UNDO
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LIB_LAYOUT_UNDO

   ``kit/src/workbench/layout/tests.rs`` --
   ``deleting_without_a_selection_does_nothing`` checks that deleting the selected
   view with none selected does nothing; ``renaming_the_selected_view_opens_its_tab``;
   ``undo_brings_back_the_layout_before_the_last_delete``,
   ``each_delete_has_its_own_undo_number`` and
   ``clearing_the_layout_can_be_undone`` the undo, its number and clearing.

.. test:: The state of the connections is read in one place
   :id: TEST_LIB_CONN_STATUS
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LIB_CONN_STATUS

   ``kit/src/workbench/connections/tests.rs::the_status_sums_up_the_connections``
   and ``clearing_the_failures_leaves_no_status`` -- the status is none, being
   opened, open, or failed with a lost connection told apart, and forgetting the
   failures leaves none.

.. test:: Time controls and log tables of an application
   :id: TEST_LIB_TIME_API
   :status: implemented
   :component: blip-kit
   :verifies: REQ_LIB_TIME_API

   ``kit/src/widgets/time/tests.rs`` --
   ``releasing_the_cursor_keeps_the_window``,
   ``play_shows_only_while_something_moves_and_needs_data`` (``toggle_play_as``
   and ``can_play``) and ``live_releases_a_set_cursor_in_a_recording``.
   ``kit/src/widgets/view/log_table/tests.rs::a_click_on_an_entry_puts_the_cursor_there``
   -- a log table given the time range sets its cursor. That a time range tells
   whether a connection is open is checked through ``the_empty_timeline_says_how_to_get_data``
   in ``kit/src/widgets/time/timeline/tests.rs``.
