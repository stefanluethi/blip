Time
====

Tests of the requirements in :doc:`/requirements/system/time`.

.. test:: Time cursor is set and dragged
   :id: TEST_BLIP_CURSOR
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_CURSOR

   ``kit/src/widgets/time.rs``, ``kit/src/widgets/view/time_axis.rs`` and
   ``kit/src/store/sorted.rs`` tests --
   keep the cursor at the end of the range until set, stop live mode when it
   is set, and click and drag it in a plot without panning the plot.
   ``kit/src/widgets/time/tests.rs::releasing_the_cursor_keeps_the_window`` --
   releasing the cursor puts it back at the end and leaves the window where
   it is. ``blip/src/app/tests.rs::escape_releases_the_cursor_once_the_connections_are_closed``
   -- Escape releases the cursor only once the connections list is closed.
   ``kit/src/widgets/view/time_axis/tests.rs::a_double_click_leaves_the_time_as_it_was``
   -- a double click on a plot leaves the range and the cursor, set or not, as
   they were.

.. test:: Recording is switched, the time range paused and played
   :id: TEST_BLIP_PLAYBACK
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_PLAYBACK

   ``blip/src/app.rs``, ``kit/src/widgets/time.rs``,
   ``kit/src/widgets/view/time_axis.rs``, ``kit/src/store/sorted.rs`` and
   ``kit/src/store.rs`` tests --
   drop values and log records while not recording but keep units,
   actions and answers, start a recording only while connected, clearing the
   data but the latest units and actions, which are recorded again, stop
   recording when a recording file is opened, keep a paused range while data
   arrives, play on from it with the cursor without skipping ahead by the
   paused time, return to live mode at the newest data, pause when panning or
   setting the cursor but not when zooming, also with the mouse wheel over a
   plot, stay unchanged when zooming beyond the shortest or longest window, and
   switch with the buttons of the bottom panel; ``kit/src/store/session/tests.rs``
   and ``blip/src/app/tests.rs::record_asks_before_it_drops_an_unsaved_recording``
   -- starting a recording asks while the data is an unsaved recording.

.. test:: Live follows the data or shows the end
   :id: TEST_BLIP_LIVE
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_LIVE

   ``kit/src/widgets/time/tests.rs`` --
   ``live_releases_a_set_cursor_in_a_recording`` checks that Live is not
   available at the end of a recording without a cursor, becomes available when
   the cursor is set, releases it, and is available while recording;
   ``the_window_shows_how_far_it_is_behind_the_newest_data`` that the panel
   tells how far the range is behind while it does not follow;
   ``the_buttons_pause_and_go_live``.

.. test:: Play is offered only where something can move
   :id: TEST_BLIP_PLAY
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_PLAY

   ``kit/src/widgets/time/tests.rs::play_shows_only_while_something_moves_and_needs_data``
   -- in an opened recording at its end nothing moves and Play is not
   available; while recording live moves on; moved back it plays from there;
   without data there is nothing to play. ``blip/src/app/tests.rs::space_plays_and_pauses``
   -- Space does what the button does.

.. test:: The window length is a field with presets
   :id: TEST_BLIP_WINDOW
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_WINDOW

   ``kit/src/widgets/time/tests.rs`` --
   ``the_window_field_keeps_and_marks_what_it_cannot_read`` checks that "5 Sec"
   sets 5 s, that "5 parsecs" stays in the field with the accepted forms on
   hover and the length unchanged, and that "30 d" is limited and marked with
   the limits on hover; ``the_window_presets_set_round_lengths``;
   ``durations_are_formatted_short_and_read_back`` that the units read in any
   case; ``leaving_the_window_field_unchanged_keeps_a_length_set_meanwhile``.

.. test:: The timeline tells about gaps, days and no data
   :id: TEST_BLIP_TIMELINE_READING
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_TIMELINE

   ``kit/src/widgets/time/timeline/tests.rs`` --
   ``pressing_on_the_ruler_within_the_window_keeps_it``;
   ``hovering_a_gap_tells_how_long_it_lasts`` ("No data for" and the length);
   ``the_first_label_of_a_new_day_has_the_date``;
   ``the_empty_timeline_says_how_to_get_data`` ("Press Record to collect
   data", and the hints for not connected and for recording). The date in the
   hover pill is checked by hand.

.. test:: Record asks for attention while data is dropped
   :id: TEST_BLIP_RECORD_HINT
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_VIS_RECORD_HINT

   ``kit/src/store/session/tests.rs::a_session_waits_for_record_while_connected_without_samples``
   -- the session waits for Record while a source is connected, not recording
   and without samples, and not while recording or once samples are shown.
   ``kit/src/widgets/time/timeline/tests.rs::the_empty_timeline_says_how_to_get_data``
   -- the timeline says so. The emphasised Record button (inverted fill, label and dot
   in the background colour, no amber, also with data shown), its red look while
   recording, its plain disabled look while not connected and its hover texts are
   checked by hand.
