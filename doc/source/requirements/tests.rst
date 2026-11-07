Test Cases
==========

.. test:: Simulator outpost streams values
   :id: TEST_OUTPOST_SIM
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM, REQ_PROTO_CBOR

   ``outpost/tests/test_simulator.py::test_simulator_outpost`` -- connects a
   WebSocket client to the outpost in simulator mode and checks that valid
   CBOR messages are received.

.. test:: Simulator outpost streams logs
   :id: TEST_OUTPOST_SIM_LOGS
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM, REQ_PROTO_CBOR

   ``outpost/tests/test_simulator.py::test_simulator_outpost_logs`` -- connects a
   WebSocket client to the outpost in simulator mode and checks that well-formed
   ``Log`` records are received.

.. test:: Simulator outpost streams typed values
   :id: TEST_OUTPOST_SIM_TYPES
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM, REQ_PROTO_TYPES

   ``outpost/tests/test_simulator.py::test_simulator_outpost_typed_values`` --
   connects a WebSocket client to the outpost in simulator mode and checks that
   records, arrays, integers, bools, strings and nulls arrive as plain CBOR
   items.

.. test:: Simulator outpost streams traces at different rates
   :id: TEST_OUTPOST_SIM_RATES
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM

   ``outpost/tests/test_simulator.py::test_simulator_outpost_rates`` -- connects
   a WebSocket client to the outpost in simulator mode and checks that faster
   sampled traces arrive more often than slower ones.

.. test:: Views compute what they show
   :id: TEST_BLIP_VIEWS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_STATES, REQ_VIS_TABLE, REQ_VIS_XY, REQ_VIS_HEATMAP

   ``blip/src/app/view`` unit tests -- merge equal values into state runs
   with gaps at nulls, hide collapsed rows of the table, pair samples with the
   current sample of their x trace, pair sibling x and y traces, sample arrays
   over time for heatmaps, and zoom plots with the mouse wheel without panning
   them, unless disabled.

.. test:: Histograms count the samples of the shown time range
   :id: TEST_BLIP_HIST
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST, REQ_VIS_HIST_EMPTY

   ``blip/src/app/view/histogram.rs::counts_add_up_to_the_samples_of_the_shown_window``
   -- the counts of a trace add up to its samples in the shown time range
   without its nulls, live, after panning and zooming, and as samples arrive.
   ``blip/src/app/view/histogram.rs::traces_share_one_set_of_bins_and_nulls_are_not_counted``
   -- two traces are counted in one set of bins, each without its nulls.
   ``blip/src/app/view/histogram.rs::numbers_and_elements_of_small_arrays_are_checked_but_not_bools_or_strings``
   -- integer and float traces and the elements of small arrays can be
   checked, bool and string traces, arrays of bools and large arrays cannot.
   ``blip/src/app/view/histogram.rs::the_legend_names_the_traces_in_the_title_row_or_in_a_corner``
   -- the title row shows the names without values and the unit only if both
   traces share it; without a title row the names are in the top right
   corner, with one they are not in the view.
   ``blip/src/app/view/histogram.rs::bins_show_the_unit_only_if_all_traces_share_one``
   -- the range of a bin has the unit only if the traces share one (the value
   axis and the bin width take the same shared unit).
   ``blip/src/app/view/histogram.rs::values_are_written_in_the_format_the_traces_share``
   -- bin edges and axis values are in the number format of the traces if all
   have the same, else in the default format, with the decimals of the step.
   ``blip/src/app/view/histogram.rs::without_traces_or_samples_the_view_says_so``
   -- without traces the view asks to check traces in the tree, with only
   nulls in the shown window it shows "No samples in the shown window".
   ``blip/src/app/view/histogram.rs::samples_of_one_value_are_one_bar_centred_on_it``
   -- samples all of one value are one bar centred on it, 1 wide for
   integers, else a round width near 1 % of the value.
   ``blip/src/app/view/histogram.rs::outlines_run_over_the_bins_that_are_not_empty``
   -- the outline of a trace runs over each run of bins that are not empty
   and drops to the floor at its ends, with nothing along the floor between
   runs and nothing for a trace without binned samples.
   ``blip/src/app/view/histogram.rs::value_labels_take_one_round_step_and_stay_within_the_axis``
   -- the value axis is labelled at one step of 1, 2 or 5 times a power of
   ten, the smallest far enough apart, with the powers of ten above it
   stronger and the grid lines between at a step that divides it, so steps of
   2 and 5 never show together; a label that would run past an end of the
   axis is left out.
   ``blip/src/app/view/histogram.rs::the_title_legend_cuts_the_names_and_keeps_the_unit``
   -- a title row too short for the legend cuts the names off before the unit,
   which stays at the end.
   ``blip/src/app/view/histogram.rs::the_legend_box_and_the_outside_counts_are_clear_of_the_bars``
   -- without a title row, the legend in its box and the unit below the names
   lie in the plot outside the reach of the bars, upright and sideways.
   ``blip/src/app/view/histogram.rs::the_legend_of_many_traces_wraps_across_the_plot_clear_of_the_bars``
   -- a legend of nine traces too tall for the room beyond the bars wraps
   into lines across the plot, at most three, clear of the bars.
   ``blip/src/app/view/histogram.rs::infinite_samples_alone_show_their_counts_on_empty_axes``
   -- a window of only infinite samples counts them below and above and shows
   the empty axes, not "No samples in the shown window".

.. test:: Histogram bins and range
   :id: TEST_BLIP_HIST_BINS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST_BINS

   ``blip/src/app/view/histogram.rs::automatic_bins_number_about_the_square_root_of_the_samples``
   -- automatic bins number about the square root of the samples, at least 8
   and at most 64, with a width of 1, 2 or 5 times a power of ten, at least a
   few points wide and at least 1 for integers.
   ``blip/src/app/view/histogram.rs::the_automatic_width_changes_only_clearly_past_a_step``
   -- as the values change around a step between two widths, the automatic
   width stays until the ideal width is about 20 % past the step, in either
   direction, and a width that is not round or too narrow is not kept.
   ``blip/src/app/view/histogram.rs::the_value_axis_fits_the_values_with_a_margin``
   -- the value axis fits the values with 5 % on each side, or a fixed range.
   ``blip/src/app/view/histogram.rs::bin_edges_are_multiples_of_the_width_and_stay_as_the_window_moves``
   -- as the values move, the width stays and the edges are multiples of it.
   ``blip/src/app/view/histogram.rs::a_fixed_number_or_width_of_bins_is_kept`` -- a fixed number of bins
   splits the range of the values evenly, a fixed width is kept, and a value
   at the end of a fixed range is in the last bin.
   ``blip/src/app/view/histogram.rs::samples_outside_a_fixed_range_are_counted_below_and_above_it``
   -- samples outside a fixed range are not binned but counted below and
   above it.
   ``blip/src/app/view/histogram.rs::infinite_samples_are_counted_beyond_the_bins_also_without_a_fixed_range``
   -- infinite samples are not binned nor in the mean but counted below and
   above the bins, also without a fixed range.
   ``blip/src/app/view/histogram.rs::bins_of_whole_widths_over_integers_are_centred_on_them``
   -- automatic and fixed width bins of a whole width over integer traces have
   their edges half a unit below multiples of the width, so the bar of 5 runs
   from 4.5 to 5.5 as when 5 is the only value, no integer is on an edge and a
   bin of width 2 holds two integers, also in a fixed range; bins of floats or
   narrower bins are not moved.

.. test:: Histogram counts axis
   :id: TEST_BLIP_HIST_COUNTS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST_COUNTS

   ``blip/src/app/view/histogram.rs::the_counts_axis_fits_the_tallest_bin_in_view``
   -- the counts axis fits the largest percentage of a trace in a bin in
   view, also of values zoomed in on, at least that of one sample, grows at
   once and shrinks only well below its top, and leaves room for the marker
   strip.
   ``blip/src/app/view/histogram.rs::the_log_counts_axis_marks_powers_of_ten_from_below_one_sample``
   -- the logarithmic axis of 1000 samples starts half a decade below one
   sample (0.1 %) and marks 0.1 %, 1 %, 10 % and 100 %, of 2000 samples it
   starts rounded down to half a decade without a mark at its start, a bin of
   one sample reaches above its start and an empty bin shows nothing; the
   linear axis marks round steps of percent, also fractions of a percent, and
   leaves the floor to the end label of the value axis.
   ``blip/src/app/view/histogram.rs::count_labels_stay_within_the_axis``
   -- a percentage label that would run past the end of the counts axis is
   left out, by half its width along a horizontal axis and half its height along a
   vertical one.
   ``blip/src/app/view/histogram.rs::the_room_beyond_the_bars_holds_the_markers_the_outside_counts_and_the_legend``
   -- the counts axis leaves room beyond the bars for the marker strip, the
   counts outside the bins and the legend box, upright and sideways, with the
   legend below the count of the samples above.

.. test:: Histogram mean and spread
   :id: TEST_BLIP_HIST_MARKERS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST_MARKERS

   ``blip/src/app/view/histogram.rs::mean_and_sigma_count_each_binned_sample_once``
   -- markers are off by default; σ is the population standard deviation;
   a sample that holds long counts once, so the mean differs from the
   time-weighted one of the plot statistics.
   ``blip/src/app/view/histogram.rs::markers_are_a_dot_or_a_dot_and_a_whisker_in_a_row_per_trace``
   -- no markers when off, a dot per trace for Mean, a dot and a whisker over
   ±σ for Mean ±σ, a row per trace, without a whisker for a trace without
   spread.

.. test:: Histogram orientation and the plot it is linked to
   :id: TEST_BLIP_HIST_LINK
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST_ORIENT, REQ_VIS_HIST_LINK

   ``blip/src/app/view/histogram.rs::upright_values_run_along_x_and_sideways_along_y``
   -- upright puts the values on the horizontal axis and sideways on the
   vertical one, with the value axis on the left by default, and only the
   value axis pans and zooms.
   ``blip/src/app/view/histogram.rs::the_value_axis_of_a_sideways_histogram_is_on_the_side_chosen``
   (``egui_kittest``) -- the value axis of a sideways histogram takes its room
   on the left or on the right, as chosen.
   ``blip/src/app/layout.rs::the_plots_of_the_layout_are_listed_with_the_y_ranges_they_show``
   -- only the plot views of the layout can be linked to, with the y range
   they show, fixed, zoomed or panned.
   ``blip/src/app/layout.rs::a_linked_histogram_takes_the_traces_of_the_plot``
   -- a linked histogram takes the traces, wildcards and number formats of
   the plot as they change, a link to another view takes nothing, and
   unlinked it keeps the traces it took.
   ``blip/src/app.rs::a_linked_histogram_shows_the_traces_of_the_plot_greyed``
   (``egui_kittest``) -- the tree of the traces shows the traces of the
   linked plot checked and greyed, with a hint naming the plot.
   ``blip/src/app/view/mod.rs::the_number_formats_of_a_linked_histogram_are_greyed``
   (``egui_kittest``) -- the number formats of a linked histogram are greyed,
   those of an unlinked one, or of one linked to a plot no longer in the
   layout, are not.
   ``blip/src/app/view/histogram.rs::a_linked_histogram_bins_the_y_range_of_the_plot_in_either_orientation``
   -- a linked histogram, sideways or upright, covers the y range of the plot
   with automatic, a fixed number of and fixed width bins, counts the samples
   below and above it and ignores its own fixed range; a plot not in the
   layout is not followed.
   ``blip/src/app/view/histogram.rs::a_linked_plot_sets_the_values_which_do_not_pan_or_zoom``
   -- in either orientation the value axis shows the plot's range as it
   changes, not its own zoom, and dragging and the wheel do not move it.
   ``blip/src/app/view/histogram.rs::the_link_is_chosen_in_either_orientation_and_greys_the_range``
   (``egui_kittest``) -- the plot is chosen in **Link** › **Plot** while
   upright, the link stays when turned sideways, and the range rows are
   greyed while linked and back once unlinked.
   ``blip/src/app/view/histogram.rs::a_link_to_a_plot_without_a_range_yet_turns_off_the_own_range_and_zoom``
   -- while linked to a plot that shows no y range yet, the own fixed range,
   the wheel zoom and the range rows are off too.
   ``blip/src/app/view/histogram.rs::plots_of_the_same_name_are_told_apart_by_a_number``
   -- plots of the same name are listed with a number to link to.
   ``blip/src/app/layout.rs::deleting_a_plot_drops_the_links_to_it`` --
   deleting the plot drops the link, deleting another view does not.

.. test:: Histogram hover, zoom and pan
   :id: TEST_BLIP_HIST_HOVER
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST_HOVER

   ``blip/src/app/view/histogram.rs::hovering_a_bin_shows_its_range_and_the_shares_and_counts_of_the_traces``
   (``egui_kittest``) -- hovering a bin highlights it and shows its range with
   the shared unit and a row per trace with its percentage and count; off the
   bins nothing is highlighted.
   ``blip/src/app/view/histogram.rs::the_tooltip_shows_the_share_and_the_count_of_each_trace``
   -- the tooltip writes the percentage of a bin and its count of all samples
   of the trace, those outside the bins included, e.g. "25 %" and "2 of 8".
   ``blip/src/app/view/histogram.rs::the_tooltip_is_as_wide_as_its_texts_wherever_it_opens``
   (``egui_kittest``) -- each time the tooltip opens it is only as wide as
   its texts, with the counts at its right edge, not as wide as a tooltip
   can be.
   ``blip/src/app/view/histogram.rs::the_names_shares_and_counts_of_a_row_of_the_tooltip_line_up``
   (``egui_kittest``) -- the name, share and count of a row share a line, and
   the shares and the counts are right aligned in their columns.
   ``blip/src/app/view/histogram.rs::bins_show_the_unit_only_if_all_traces_share_one``
   -- the range has the unit only if all traces share one.
   ``blip/src/app/view/histogram.rs::the_wheel_zooms_and_a_drag_pans_the_values_without_binning_anew``
   (``egui_kittest``) -- the wheel zooms and a drag pans the value axis
   without changing the bins, a double click fits it again, and none of them
   changes the shown time range or the time cursor.

.. test:: Histogram settings are saved with the layout
   :id: TEST_BLIP_HIST_SAVED
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST_SAVED

   ``blip/src/app/layout/file.rs::layouts_survive_json`` -- saves a layout
   with a histogram whose bins, range, counts scale, markers, orientation,
   value axis side and linked plot differ from the defaults as JSON and
   checks that it loads unchanged.
   ``blip/src/app/layout.rs::links_between_views_survive_saving_and_loading``
   -- the linked plot is saved by its place in the layout and is the same
   plot after loading.
   ``blip/src/app/view/histogram.rs::settings_are_saved_with_defaults_for_those_missing``
   -- the settings survive JSON, and a histogram saved without them loads
   with the defaults.

.. test:: Histogram screenshots
   :id: TEST_BLIP_HIST_SHOTS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST, REQ_VIS_HIST_MARKERS, REQ_VIS_HIST_ORIENT

   ``blip/src/app/screenshots.rs::screenshots_views`` renders
   ``view-histogram``, an upright histogram of ``robot/pose.x`` and
   ``robot/pose.y`` with Mean ±σ markers, and
   ``blip/src/app/screenshots.rs::screenshots_layouts`` renders
   ``view-histogram-sideways``, a sideways histogram of
   ``motor.speed_rpm`` beside the plot it is linked to, from the demo recording in
   the light and the dark theme, for the documentation and to check by eye
   that they look right.

.. test:: Log records are kept by path and shown in a table
   :id: TEST_BLIP_LOGS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_LOG

   ``blip/src/app/model/logs.rs`` and ``blip/src/app/view/log_table.rs`` unit
   tests -- keep records by path in the order of their times, changing the
   order of a path only when a record arrives before others of it; show the
   records of the picked paths merged by time, filter only new records unless
   the filter or the paths change, filter by level, component and message, show
   the path while several are shown, and show or hide the date and the
   component as the properties say.

.. test:: Time cursor is set and dragged
   :id: TEST_BLIP_CURSOR
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_CURSOR

   ``blip/src/app/time.rs`` and ``blip/src/app/view/time_axis.rs`` tests --
   keep the cursor at the end of the range until set, stop live mode when it
   is set, and click and drag it in a plot without panning the plot.

.. test:: Recording is switched, the time range paused and played
   :id: TEST_BLIP_PLAYBACK
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_PLAYBACK

   ``blip/src/app.rs``, ``blip/src/app/time.rs``,
   ``blip/src/app/view/time_axis.rs`` and ``blip/src/app/model.rs`` tests --
   drop values and log records while not recording but keep units,
   actions and answers, start a recording only while connected, clearing the
   data but the latest units and actions, which are recorded again, stop
   recording when a recording file is opened, keep a paused range while data
   arrives, play on from it with the cursor without skipping ahead by the
   paused time, return to live mode at the newest data, pause when panning or
   setting the cursor but not when zooming, also with the mouse wheel over a
   plot, stay unchanged when zooming beyond the shortest or longest window, and
   switch with the buttons of the bottom panel.

.. test:: Trace values encode as plain CBOR
   :id: TEST_PROTO_DATA
   :status: implemented
   :component: proto
   :verifies: REQ_PROTO_TYPES, REQ_PROTO_CBOR

   ``proto/src/data.rs`` unit tests -- encode every kind of ``Data`` to the
   expected CBOR bytes, round-trip nested arrays and records, decode values
   encoded by the Python outpost, and reject byte strings, non-text map keys,
   duplicate fields and integers out of range.

.. test:: Frames are split into their messages
   :id: TEST_BLIP_CBOR_SEQUENCE
   :status: implemented
   :component: blip
   :verifies: REQ_PROTO_SEQUENCE

   ``blip/src/app/model.rs::a_cbor_sequence_is_split_into_its_items`` --
   splits a CBOR sequence of a value and a log record into both messages, and
   one cut off or broken in the middle into the messages before and an error.

.. test:: Integers are kept exactly
   :id: TEST_BLIP_NUMBERS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_INT

   ``blip/src/app/model/numbers.rs`` and ``blip/src/app/model/traces.rs``
   tests -- keep integers beyond 2^53 exact, switch to unsigned for integers
   above ``i64::MAX``, reject integers that do not fit with the others, and
   write them in every number format.

.. test:: Connections are edited, opened and their errors notified
   :id: TEST_BLIP_CONNECTIONS
   :status: implemented
   :component: blip
   :verifies: REQ_CONN_WS, REQ_CONN_ERRORS

   ``blip/src/app/connections.rs``, ``blip/src/app/toasts.rs`` and
   ``blip/src/app.rs`` tests -- name connections after their address, add
   them with the + button, open several at once, pass on when the first one
   opens before its messages and start afresh then unless recording, keep
   them from being changed while recording but connect again those that
   broke, ask before dropping an unsaved recording, report why an invalid
   address fails to connect, show errors and warnings as toasts until they
   expire or are closed, count a repeated one instead of stacking it, and
   warn when a recording is opened in part; ``blip/src/app/model/traces.rs``
   tests -- put the samples of a path in the order of their timestamps,
   also when several connections send it.

.. test:: Samples stay on one timeline
   :id: TEST_TIMELINE
   :status: implemented
   :component: blip, outpost
   :verifies: REQ_CONN_TIMELINE

   ``blip/src/app/model.rs`` tests -- drop values and log records of another
   time base than the first sample, move records without a timestamp to a
   first timestamp in another time base, and start afresh when cleared;
   ``outpost/tests/test_timing.py`` -- keep the time of a target, shift a run
   after a jump back, a short run, or an earlier time after a new connection
   or a sequence reset to follow the newest time plus the time since, but not
   a late or a little out of order sample, shift timestamps of another clock
   alike, and change the timestamp of a message in place wherever it is in
   the map; ``test_protobuf.py`` and ``test_socket.py`` -- shift the times
   after a restart of a protobuf or socket target and those of the bridge,
   log it, keep a clock per host, and drop untimed values, also of the script,
   before the first timestamp.

.. test:: Recordings are saved and opened
   :id: TEST_BLIP_RECORDING
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_RECORDING

   ``blip/src/app/model/recording.rs`` and ``blip/src/app.rs`` tests -- save
   frames and read them back unchanged, read uncompressed files, files of
   several gzip members and files written by the Python outpost, read a cut
   off file up to the cut and reject other files; open a recording in place
   of the data and units before, and with the traces panel started anew.

.. test:: Presets are exported and imported as JSON
   :id: TEST_BLIP_PRESETS_FILE
   :status: implemented
   :component: blip
   :verifies: REQ_FILE_PRESETS

   ``blip/src/app/layout/file.rs::layouts_survive_json`` -- saves a layout
   with a view of each type as JSON and checks it loads unchanged.
   ``blip/src/app/presets.rs`` tests -- write presets with their layout and
   connections to a file and read them back unchanged, export the chosen
   presets, import replacing those of the same names, refuse files of a newer
   version, layout files and presets without or with the same names, keep the
   connections read apart, refuse an import while a dialog is open, leave out
   the active preset while recording and take it once recording stops, keep
   kept presets that cannot be read, read the demo presets.
   ``blip/src/app.rs::a_presets_file_is_imported_as_chosen_in_its_dialog`` --
   opens a presets file, imports it from its dialog and shows the replaced
   active preset.

.. test:: Presets are switched and kept
   :id: TEST_BLIP_PRESETS
   :status: implemented
   :component: blip
   :verifies: REQ_PRESETS, REQ_PERSIST_STATE

   ``blip/src/app.rs`` tests -- switching, adding, duplicating and deleting
   presets shows the layout and connections of each, and the presets menu
   does not switch while recording. ``blip/src/app/presets.rs`` tests --
   the presets are kept with the active one, kept presets of a newer version
   are not read.
   ``blip/src/app/connections.rs::connections_of_a_preset_keep_their_addresses_and_names_apart``
   -- the connections of a preset keep their addresses and names apart.

.. test:: Outpost records and replays
   :id: TEST_OUTPOST_RECORDING
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_RECORD, REQ_OUTPOST_REPLAY

   ``outpost/tests/test_recording.py`` -- writes and reads recording files,
   also cut off and uncompressed ones, and replays a recording through the
   outpost to a WebSocket client while recording what is sent, checking that
   both are the recorded frames byte for byte.

.. test:: Outpost splits the batches of socket targets
   :id: TEST_OUTPOST_SOCKET
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SOCKET

   ``outpost/tests/test_socket.py`` -- forwards several messages of one TCP
   read and of one UDP datagram one by one, a message split over reads once
   it is complete, keeps an ``Actions`` and drops an ``ActionResult`` within a
   batch, drops undecodable data and messages that do not end, and keeps
   reconnecting after errors.

.. test:: Action messages encode as plain CBOR
   :id: TEST_PROTO_ACTIONS
   :status: implemented
   :component: proto
   :verifies: REQ_CTRL_ANNOUNCE, REQ_CTRL_FEEDBACK

   ``proto/src/action.rs`` unit tests -- encode invocations and results as
   the documented CBOR maps, parameter kinds as strings or maps, and decode
   announcements whose optional fields are left out.

.. test:: Outpost announces, checks and carries out actions
   :id: TEST_OUTPOST_ACTIONS
   :status: implemented
   :component: outpost
   :verifies: REQ_CTRL_ANNOUNCE, REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK, REQ_OUTPOST_PROTOCOL

   ``outpost/tests/test_actions.py`` -- takes the parameters of actions from
   their type hints and rejects unsupported ones, checks and converts
   arguments, rejects disabled actions and those that raise, sends the
   actions to connecting clients and changed ones to all, answers only the
   invoking client, logs to all, and runs a slow action without holding up
   others.

.. test:: Outpost serves a script
   :id: TEST_OUTPOST_LIBRARY
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_PROTOCOL, REQ_OUTPOST_RECORD

   ``outpost/tests/test_outpost.py`` -- serves from a thread of its own and
   passes on what another thread sends, in order; reports a port in use;
   sends the units and actions sent before to blips connecting later and to
   the recording; announces actions changed from another thread; finishes the
   recording on stop; and makes functions actions right away and methods those
   of registered objects.

.. test:: Tracers, batches and records
   :id: TEST_OUTPOST_TRACERS
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_BATCH, REQ_OUTPOST_TRACERS

   ``outpost/tests/test_tracer.py`` -- the same name gives the same tracer, its
   traces, actions and readbacks are below its name and its log records at
   its name and of its component, formatted as ``logging`` does; forwarded
   log records carry their own path and component; a dataclass is a record
   whose units are sent once per path, also of the fields whose annotations
   resolve when others do not; numpy-like values are sent as lists; a series
   is sent at its timestamps or its period; a batch is handed over at its end,
   also when it raises, in one handover from another thread, and with one
   timestamp, shared by the batches nested in it; a single mapping formats by
   name; ``LogHandler`` maps the levels of ``logging`` and
   skips the outpost's own records. ``outpost/tests/test_app.py`` -- the
   example ``app.py`` traces its parts below their names.

.. test:: Simulator actions change what it sends
   :id: TEST_OUTPOST_SIM_ACTIONS
   :status: implemented
   :component: outpost
   :verifies: REQ_OUTPOST_SIM

   ``outpost/tests/test_simulator.py::test_simulator_actions_change_what_it_sends``
   -- the motor follows its setpoint and rejects it while disabled, the log
   level filters log records and is sent as its readback, and the
   calibration is disabled while it runs.

.. test:: blip invokes actions and shows their outcome
   :id: TEST_BLIP_ACTIONS
   :status: implemented
   :component: blip
   :verifies: REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK, REQ_CTRL_VIEW, REQ_CTRL_SAFE

   ``blip/src/app/model/actions.rs``, ``blip/src/app/view/actions.rs`` and
   ``blip/src/app/view/trace_selection.rs`` tests -- send invocations with new
   ids, show only the outcome of the latest invocation of an action of an
   outpost, time out unanswered ones, send at most every interval while
   dragging and the final value on release; send buttons and checkboxes right
   away and actions with several parameters with their Send button, ask before
   confirmed actions, disable actions while disconnected or disabled, and
   start inputs from the readback or the default; send the actions to the
   outpost chosen for the view, by default each to the first announcing it
   with an open connection, show and follow the announced readback unless
   turned off, choose both in the properties, keep the inputs per outpost,
   show no actions in new views until checked, list the actions in the
   tree of the traces, also those shown but not announced, and check them
   there only for actions views.

.. test:: Documentation builds without warnings
   :id: TEST_DOC_BUILD
   :status: implemented
   :component: doc
   :verifies: REQ_DOC_BUILD

   ``cd doc && make html`` -- builds with ``-W --keep-going``, so a broken
   reference or a malformed directive fails the build; the Pages workflow
   (``.github/workflows/pages.yml``) runs it for each published change.

.. test:: Every option of the outpost command line is documented
   :id: TEST_DOC_CLI
   :status: implemented
   :component: doc
   :verifies: REQ_DOC_CLI_REF

   ``outpost/tests/test_docs.py::test_every_option_is_documented`` and
   ``::test_every_command_is_documented`` -- go through the commands and options
   of ``blip-outpost`` and check that each is on the page of the command line.

.. test:: The examples of the outpost guide work
   :id: TEST_DOC_EXAMPLES
   :status: implemented
   :component: doc
   :verifies: REQ_DOC_EXAMPLES

   ``outpost/tests/test_docs.py::test_the_python_examples_of_the_guide_compile``
   and ``::test_the_minimal_example_sends_values`` -- compile each Python code
   block of the outpost guide, and run ``examples/minimal.py``, which the guide
   includes, until it sends a value to a connected client.

.. test:: Links to other sites work
   :id: TEST_DOC_LINKS
   :status: implemented
   :component: doc
   :verifies: REQ_DOC_LINKS

   ``cd doc && make linkcheck`` -- requests each link to another site and fails
   on broken ones; run before each release.
