Histogram
=========

Tests of the requirements in :doc:`/requirements/system/histogram`.

.. test:: Histograms count the samples of the shown time range
   :id: TEST_BLIP_HIST
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST, REQ_VIS_HIST_EMPTY

   ``blip/src/app/view/distribution.rs::counts_add_up_to_the_samples_of_the_shown_window``
   -- the counts of a trace add up to its samples in the shown time range
   without its nulls, live, after panning and zooming, and as samples arrive.
   ``blip/src/app/view/distribution.rs::traces_share_one_set_of_bins_and_nulls_are_not_counted``
   -- two traces are counted in one set of bins, each without its nulls.
   ``blip/src/app/view/distribution.rs::numbers_and_elements_of_small_arrays_are_checked_but_not_bools_or_strings``
   -- integer and float traces and the elements of small arrays can be
   checked, bool and string traces, arrays of bools and large arrays cannot.
   ``blip/src/app/view/distribution.rs::the_legend_names_the_traces_in_the_title_row_or_in_a_corner``
   -- the title row shows the names without values and the unit only if both
   traces share it; without a title row the names are in the top right
   corner, with one they are not in the view.
   ``blip/src/app/view/distribution.rs::bins_show_the_unit_only_if_all_traces_share_one``
   -- the range of a bin has the unit only if the traces share one (the value
   axis and the bin width take the same shared unit).
   ``blip/src/app/view/distribution.rs::values_are_written_in_the_format_the_traces_share``
   -- bin edges and axis values are in the number format of the traces if all
   have the same, else in the default format, with the decimals of the step.
   ``blip/src/app/view/distribution.rs::without_traces_or_samples_the_view_says_so``
   -- without traces the view asks to check traces in the tree, with only
   nulls in the shown window it shows "No samples in the shown window".
   ``blip/src/app/view/distribution.rs::samples_of_one_value_are_one_bar_centred_on_it``
   -- samples all of one value are one bar centred on it, 1 wide for
   integers, else a round width near 1 % of the value.
   ``blip/src/app/view/distribution.rs::outlines_run_over_the_bins_that_are_not_empty``
   -- the outline of a trace runs over each run of bins that are not empty
   and drops to the floor at its ends, with nothing along the floor between
   runs and nothing for a trace without binned samples.
   ``blip/src/app/view/distribution.rs::the_title_legend_cuts_the_names_and_keeps_the_unit``
   -- a title row too short for the legend cuts the names off before the unit,
   which stays at the end.
   ``blip/src/app/view/distribution.rs::infinite_samples_alone_show_their_counts_on_empty_axes``
   -- a window of only infinite samples counts them below and above and shows
   the empty axes, not "No samples in the shown window".

.. test:: Histogram bins and range
   :id: TEST_BLIP_HIST_BINS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST_BINS

   ``blip/src/app/view/distribution.rs::automatic_bins_number_about_the_square_root_of_the_samples``
   -- automatic bins number about the square root of the samples, at least 8
   and at most 64, with a width of 1, 2 or 5 times a power of ten, at least a
   few points wide and at least 1 for integers.
   ``blip/src/app/view/distribution.rs::the_automatic_width_changes_only_clearly_past_a_step``
   -- as the values change around a step between two widths, the automatic
   width stays until the ideal width is about 20 % past the step, in either
   direction, and a width that is not round or too narrow is not kept.
   ``blip/src/app/view/distribution.rs::the_value_axis_fits_the_values_with_a_margin``
   -- the value axis fits the values with 5 % on each side, or a fixed range.
   ``blip/src/app/view/distribution.rs::bin_edges_are_multiples_of_the_width_and_stay_as_the_window_moves``
   -- as the values move, the width stays and the edges are multiples of it.
   ``blip/src/app/view/distribution.rs::a_fixed_number_or_width_of_bins_is_kept`` -- a fixed number of bins
   splits the range of the values evenly, a fixed width is kept, and a value
   at the end of a fixed range is in the last bin.
   ``blip/src/app/view/distribution.rs::samples_outside_a_fixed_range_are_counted_below_and_above_it``
   -- samples outside a fixed range are not binned but counted below and
   above it.
   ``blip/src/app/view/distribution.rs::infinite_samples_are_counted_beyond_the_bins_also_without_a_fixed_range``
   -- infinite samples are not binned nor in the mean but counted below and
   above the bins, also without a fixed range.
   ``blip/src/app/view/distribution.rs::bins_of_whole_widths_over_integers_are_centred_on_them``
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

   ``blip/src/app/view/distribution.rs::the_counts_axis_fits_the_tallest_bin_in_view``
   -- the counts axis fits the largest percentage of a trace in a bin in
   view, also of values zoomed in on, at least that of one sample, grows at
   once and shrinks only well below its top, and leaves room for the marker
   strip.
   ``blip/src/app/view/distribution.rs::the_log_counts_axis_marks_powers_of_ten_from_below_one_sample``
   -- the logarithmic axis of 1000 samples starts half a decade below one
   sample (0.1 %) and marks 0.1 %, 1 %, 10 % and 100 %, of 2000 samples it
   starts rounded down to half a decade without a mark at its start, a bin of
   one sample reaches above its start and an empty bin shows nothing; the
   linear axis is marked up to the bars, also in fractions of a percent, and
   leaves the floor to the end label of the value axis.
   ``blip/src/app/view/distribution.rs::the_room_beyond_the_bars_holds_the_markers_and_the_outside_counts``
   -- the counts axis leaves room beyond the bars for the marker strip and the
   counts outside the bins, upright and sideways, with the counts in the
   corners beyond the strip.

.. test:: Histogram mean and spread
   :id: TEST_BLIP_HIST_MARKERS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST_MARKERS

   ``blip/src/app/view/distribution.rs::mean_and_sigma_count_each_binned_sample_once``
   -- markers are off by default; σ is the population standard deviation;
   a sample that holds long counts once, so the mean differs from the
   time-weighted one of the plot statistics.
   ``blip/src/app/view/distribution.rs::markers_are_a_dot_or_a_dot_and_a_whisker_in_a_row_per_trace``
   -- no markers when off, a dot per trace for Mean, a dot and a whisker over
   ±σ for Mean ±σ, a row per trace, without a whisker for a trace without
   spread.

.. test:: Histogram orientation and the plot it is linked to
   :id: TEST_BLIP_HIST_LINK
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST_ORIENT, REQ_VIS_HIST_LINK

   ``blip/src/app/view/distribution.rs::upright_values_run_along_x_and_sideways_along_y``
   -- upright puts the values on the horizontal axis and sideways on the
   vertical one, with the value axis on the left by default, and only the
   value axis pans and zooms.
   ``blip/src/app/view/distribution.rs::the_value_axis_of_a_sideways_histogram_is_on_the_side_chosen``
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
   ``blip/src/app/view/distribution.rs::a_linked_histogram_bins_the_y_range_of_the_plot_in_either_orientation``
   -- a linked histogram, sideways or upright, covers the y range of the plot
   with automatic, a fixed number of and fixed width bins, counts the samples
   below and above it and ignores its own fixed range; a plot not in the
   layout is not followed.
   ``blip/src/app/view/distribution.rs::a_linked_plot_sets_the_values_which_do_not_pan_or_zoom``
   -- in either orientation the value axis shows the plot's range as it
   changes, not its own zoom, and dragging and the wheel do not move it.
   ``blip/src/app/view/distribution.rs::the_link_is_chosen_in_either_orientation_and_greys_the_range``
   (``egui_kittest``) -- the plot is chosen in **Link** › **Plot** while
   upright, the link stays when turned sideways, and the range rows are
   greyed while linked and back once unlinked.
   ``blip/src/app/view/distribution.rs::a_link_to_a_plot_without_a_range_yet_turns_off_the_own_range_and_zoom``
   -- while linked to a plot that shows no y range yet, the own fixed range,
   the wheel zoom and the range rows are off too.
   ``blip/src/app/view/distribution.rs::plots_of_the_same_name_are_told_apart_by_a_number``
   -- plots of the same name are listed with a number to link to.
   ``blip/src/app/layout.rs::deleting_a_plot_drops_the_links_to_it`` --
   deleting the plot drops the link, deleting another view does not.

.. test:: Histogram hover, zoom and pan
   :id: TEST_BLIP_HIST_HOVER
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST_HOVER

   ``blip/src/app/view/distribution.rs::hovering_a_bin_shows_its_range_and_the_shares_and_counts_of_the_traces``
   (``egui_kittest``) -- hovering a bin highlights it and shows its range with
   the shared unit and a row per trace with its percentage and count; off the
   bins nothing is highlighted.
   ``blip/src/app/view/distribution.rs::the_tooltip_shows_the_share_and_the_count_of_each_trace``
   -- the tooltip writes the percentage of a bin and its count of all samples
   of the trace, those outside the bins included, e.g. "25 %" and "2 of 8".
   ``blip/src/app/view/distribution.rs::the_tooltip_is_as_wide_as_its_texts_wherever_it_opens``
   (``egui_kittest``) -- each time the tooltip opens it is only as wide as
   its texts, with the counts at its right edge, not as wide as a tooltip
   can be.
   ``blip/src/app/view/distribution.rs::the_names_shares_and_counts_of_a_row_of_the_tooltip_line_up``
   (``egui_kittest``) -- the name, share and count of a row share a line, and
   the shares and the counts are right aligned in their columns.
   ``blip/src/app/view/distribution.rs::bins_show_the_unit_only_if_all_traces_share_one``
   -- the range has the unit only if all traces share one.
   ``blip/src/app/view/distribution.rs::the_wheel_zooms_and_a_drag_pans_the_values_without_binning_anew``
   (``egui_kittest``) -- the wheel zooms and a drag pans the value axis
   without changing the bins, a double click fits it again, and none of them
   changes the shown time range or the time cursor.

.. test:: Histogram settings are saved with the layout
   :id: TEST_BLIP_HIST_SAVED
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST_SAVED, REQ_VIS_DIST_SHOW, REQ_VIS_DIST_WHISKERS

   ``blip/src/app/layout/file.rs::layouts_survive_json`` -- saves a layout
   with a histogram whose bins, range, counts scale, markers, orientation,
   value axis side and linked plot differ from the defaults as JSON and
   checks that it loads unchanged.
   ``blip/src/app/layout.rs::links_between_views_survive_saving_and_loading``
   -- the linked plot is saved by its place in the layout and is the same
   plot after loading.
   ``blip/src/app/view/distribution.rs::settings_are_saved_with_defaults_for_those_missing``
   -- the settings survive JSON, and a histogram saved without them loads
   with the defaults.
   ``blip/src/app/view/distribution.rs::show_and_whiskers_are_saved_and_old_histograms_load_as_bars``
   -- Show and Whiskers survive JSON (also in
   ``blip/src/app/layout/file.rs::layouts_survive_json``), and a view saved
   as a Histogram (the old name) loads as a Distribution view with Bars and
   1.5×IQR whiskers and its own settings, and is saved by the new name.

.. test:: Histogram screenshots
   :id: TEST_BLIP_HIST_SHOTS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HIST, REQ_VIS_HIST_MARKERS, REQ_VIS_HIST_ORIENT, REQ_VIS_DIST_SHOW, REQ_VIS_DIST_BOX

   ``blip/src/app/screenshots.rs::screenshots_views`` renders
   ``view-distribution``, the upright bars of ``robot/pose.x`` and
   ``robot/pose.y`` with Mean ±σ markers, and
   ``blip/src/app/screenshots.rs::screenshots_layouts`` renders
   ``view-distribution-sideways``, the sideways bars of
   ``motor.speed_rpm`` beside the plot it is linked to, from the demo recording in
   the light and the dark theme, for the documentation and to check by eye
   that they look right.
