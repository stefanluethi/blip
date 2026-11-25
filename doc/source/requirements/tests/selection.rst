Traces tree
===========

Tests of the requirements in :doc:`/requirements/system/selection`.

.. test:: The tree marks wildcards and picks one trace with radio buttons
   :id: TEST_BLIP_TREE_MARKS
   :status: implemented
   :component: blip
   :verifies: REQ_SEL_MARKS

   ``kit/src/widgets/view/trace_selection/tests.rs`` --
   ``wildcard_folders_are_marked_and_tell_their_state`` checks the faint "+"
   of a folder that picks what is below it and its hover;
   ``branches_and_wildcards_below_others_are_known``;
   ``one_trace_views_pick_with_radio_buttons``. The sentence case of the titles
   and the cut of long names are checked by hand.

.. test:: The tree is narrowed and opened in bulk
   :id: TEST_BLIP_TREE_FILTER
   :status: implemented
   :component: blip
   :verifies: REQ_SEL_FILTER

   ``kit/src/widgets/view/trace_selection/tests.rs`` --
   ``checked_only_shows_the_checked_and_hidden_ones_are_counted``
   checks the toggle and the count of checked traces that the search hides;
   ``alt_click_and_the_menu_open_and_close_all_below`` the Alt+click and the
   menu Expand all and Collapse all; ``without_a_view_the_folders_still_open_and_close``
   that without a selected view the folders still open.

.. test:: The tree is used from the keyboard
   :id: TEST_BLIP_TREE_KEYS
   :status: implemented
   :component: blip
   :verifies: REQ_SEL_KEYS

   ``kit/src/widgets/view/trace_selection/tests.rs`` --
   ``ctrl_f_focuses_the_search`` checks Ctrl+F;
   ``the_arrow_keys_move_the_focus_and_open_and_close_folders`` that the arrows
   open and close a folder and Space checks the row in focus.
