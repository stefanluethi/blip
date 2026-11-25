Views
=====

Tests of the requirements in :doc:`/requirements/system/views`.

.. test:: Views compute what they show
   :id: TEST_BLIP_VIEWS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_STATES, REQ_VIS_TABLE, REQ_VIS_XY, REQ_VIS_HEATMAP

   ``kit/src/widgets/view`` unit tests -- merge equal values into state runs
   with gaps at nulls, cut the names of the rows of States with "…" to the
   width of the axis (``long_names_are_cut_off_to_the_width_of_the_axis``), hide
   collapsed rows of the table, pair samples with the
   current sample of their x trace, pair sibling x and y traces, sample arrays
   over time for heatmaps, and zoom plots with the mouse wheel without panning
   them, unless disabled.

.. test:: Log records are kept by path and shown in a table
   :id: TEST_BLIP_LOGS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_LOG

   ``kit/src/store/logs.rs`` and ``kit/src/widgets/view/log_table.rs`` unit
   tests -- keep records by path in the order of their times, changing the
   order of a path only when a record arrives before others of it; show the
   records of the picked paths merged by time, filter only new records unless
   the filter or the paths change, filter by level and message, always show the
   path, show or hide the date as the properties say, and
   show no component column or property.

.. test:: Log paths are defaulted, picked and filtered
   :id: TEST_BLIP_LOG_PATHS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_LOG_DEFAULT, REQ_VIS_LOG_PICK, REQ_VIS_LOG_PATH_FILTER

   ``kit/src/store/logs``, ``kit/src/store``,
   ``kit/src/widgets/view/trace_selection`` and ``kit/src/widgets/view/log_table``
   unit tests -- checks that a record without a path is kept at ``log/default``
   and one with a path at exactly that path (``log/imu``, ``motor``); that
   checking the folder ``log`` picks the log paths below it, also those that
   appear after; that the path filter lists the paths of the shown records
   relative to the folder they share, hides and shows paths by checkbox, and
   checks all or none with its buttons; that the hidden paths are saved in the
   layout and restored, and a path that appears later is shown; that clearing
   the filters clears the path filter; and that a layout with hidden components
   loads without them.

.. test:: Integers are kept exactly
   :id: TEST_BLIP_NUMBERS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_INT

   ``kit/src/store/numbers.rs`` and ``kit/src/store/traces.rs``
   tests -- keep integers beyond 2^53 exact, switch to unsigned for integers
   above ``i64::MAX``, reject integers that do not fit with the others, and
   write them in every number format.

.. test:: Tooltips of the plot-like views
   :id: TEST_BLIP_TOOLTIP
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_TOOLTIP, REQ_VIS_DIST_BOX_HOVER, REQ_VIS_HIST_HOVER

   ``kit/src/widgets/view/tooltip`` unit tests (``egui_kittest``) -- the shared
   tooltip helper shows a header (a text in weak colour, or a trace dot and
   name), a row per entry with the dot and name or a weak label on the left, and
   the values right aligned in columns as wide as the widest value, with the
   unit, a missing or null value in the faint colour, and a rule between two
   groups of rows.
   ``kit/src/widgets/view/plot`` unit tests (``egui_kittest``) -- hovering Plot
   shows the time as header and a row per trace with its value, through the
   helper.
   ``kit/src/widgets/view/states`` unit tests (``egui_kittest``) -- hovering States
   shows the time as header and a row per trace with its state, through the
   helper.
   ``kit/src/widgets/view/heatmap`` unit tests (``egui_kittest``) -- hovering the
   bands of a Heatmap shows the time as header and the element index with its
   value, and hovering the grid the row and column as header and the value,
   through the helper.
   ``kit/src/widgets/view/heatmap`` unit tests (``egui_kittest``) -- hovering the
   array line of a Heatmap shows the element index and its value in a framed
   tooltip, and no label of egui_plot.
   ``kit/src/widgets/view/xy`` unit tests (``egui_kittest``) -- hovering XY shows
   the name of the trace with its x and y values in a framed tooltip, and no
   label of egui_plot.
   ``kit/src/widgets/view/distribution`` unit tests (``egui_kittest``) -- the
   tooltips of a bin and of a box of Distribution are built through the helper,
   with the bin range or the dot and name of the trace as header.
   ``kit/src/widgets/view/time_axis/tests.rs::time_decimals_follow_the_zoom`` --
   the time in the header has no decimals for seconds per point and up to
   nanoseconds for a deeply zoomed plot.

.. test:: Tooltip screenshots
   :id: TEST_BLIP_TOOLTIP_SHOTS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_TOOLTIP

   ``blip/src/app/screenshots.rs::screenshots_views`` renders
   ``view-plot-tooltip`` and ``view-heatmap-grid-tooltip``, a tooltip of Plot and
   one of a Heatmap, in the light and the dark theme, for the documentation and
   to check by eye that they look like the tooltip of a box of Distribution
   (``view-distribution-box-tooltip``).

.. test:: Log tables follow, mark the cursor and keep rows whole
   :id: TEST_BLIP_LOG_FOLLOW
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_LOG_FOLLOW, REQ_VIS_LOG_CURSOR, REQ_VIS_LOG_ROWS

   ``kit/src/widgets/view/log_table/tests.rs`` --
   ``scrolling_up_stops_following_until_back_at_the_bottom`` checks that the
   newest record is followed while live until the user scrolls up, and again
   at the bottom; ``a_moved_range_scrolls_only_while_following`` that a range
   that moves scrolls to its newest record only with autoscroll on;
   ``a_followed_log_shows_whole_rows_only``; ``a_click_on_an_entry_puts_the_cursor_there``;
   ``the_entry_at_a_cursor_put_elsewhere_scrolls_into_view`` that the record at
   the cursor is marked and scrolled to; ``long_messages_are_cut_to_one_line``;
   ``logs_without_entries_or_checked_logs_show_a_hint`` ("Check the logs to show
   in the tree on the left" and "No entries in this time range"). The context
   menu with Copy message and Copy row is checked by hand.

.. test:: Views without traces say what to check
   :id: TEST_BLIP_EMPTY
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_EMPTY

   ``kit/src/widgets/view/distribution/tests.rs::without_traces_or_samples_the_view_says_so``
   -- a Distribution view without traces shows "Check the traces to show in the
   tree on the left". ``kit/src/widgets/view/xy/tests.rs::a_single_array_is_plotted_over_its_indices``
   -- an XY view with one trace that is no array shows "Check a second trace to
   plot against the first", one with a single array shows no hint.
