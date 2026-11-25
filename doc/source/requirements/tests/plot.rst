Plot and ranges
===============

Tests of the requirements in :doc:`/requirements/system/plot`.

.. test:: Ends of an axis are set alone and marked when wrong
   :id: TEST_BLIP_RANGE
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_RANGE

   ``kit/src/widgets/view/tests.rs`` -- ``an_end_is_a_finite_number_or_nothing``
   checks that a field sets an end only with a finite number and is empty to
   fit; ``text_that_is_no_end_stays_and_keeps_the_end`` that text that is no
   number stays in the field and leaves the end as it was;
   ``ends_out_of_order_blame_the_ends_set`` that ends set the wrong way round
   are blamed with the reason ("Min must be below Max, 4").
   ``kit/src/widgets/view/tests.rs::ends_over_fit_the_ends_that_are_not_set``
   and ``the_hints_of_min_and_max_are_the_ends_of_the_axis`` -- Plot fits the end
   not set and shows the ends in use as hints.
   ``kit/src/widgets/view/xy/tests.rs::an_axis_is_set_only_by_ends_in_order`` --
   an XY axis takes the ends set only in order, else it fits.
   ``kit/src/widgets/view/heatmap/tests.rs::each_end_of_the_colour_scale_is_set_alone_and_wrong_ends_are_left_out``
   and ``kit/src/widgets/view/distribution/tests.rs::each_end_of_the_range_is_set_alone_and_wrong_ends_are_left_out``
   -- the same for the colour scale and for the values binned.
   ``heatmaps_saved_with_a_fixed_or_no_range_load_to_the_same_scale``,
   ``distributions_saved_with_a_fixed_or_no_range_load_to_the_same_bins`` and
   ``xy_plots_saved_before_the_new_properties_load_to_the_same_picture`` --
   presets saved with a fixed range, or none, load to the same picture. The
   outline in the error colour is checked by hand.

.. test:: Double click and drag on a plot, zoomed axis
   :id: TEST_BLIP_PLOT_AXES
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_PLOT_AXES

   ``kit/src/widgets/view/time_axis/tests.rs`` --
   ``a_double_click_leaves_the_time_as_it_was`` checks that a double click
   leaves a live range live and a cursor and range that were set as they were;
   ``a_vertical_drag_pans_no_time`` that a drag that goes vertically pans no
   time. ``kit/src/widgets/view/plot/tests.rs`` --
   ``a_vertical_drag_pans_the_y_axis_and_a_double_click_fits_it_again`` checks
   that a horizontal drag pans the time and a vertical one the y axis, and that
   a double click fits the y axis and leaves the time and the cursor. The row
   Zoomed with its button, and the zoomed range shown as the hints, are checked
   by hand.
   ``kit/src/widgets/view/heatmap/tests.rs::the_y_axis_at_the_cursor_stays_where_zoomed_until_a_double_click``
   -- the y axis of the array at the cursor stays where zoomed until a double
   click.

.. test:: Legends cannot hide traces and fit their room
   :id: TEST_BLIP_LEGEND
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_LEGEND

   ``kit/src/widgets/view/plotting/tests.rs::title_legend_entries_are_left_out_only_when_they_do_not_fit``
   -- whole entries are kept while they fit, the last one is cut where its name
   keeps enough room, else entries are left out for "…".
   ``kit/src/widgets/view/xy/tests.rs::the_legend_goes_to_a_corner_without_a_title_row``
   and ``kit/src/widgets/view/distribution/tests.rs::the_legend_names_the_traces_in_the_title_row_or_in_a_corner``
   -- the legend in a corner is painted only, so a click cannot hide a trace.
   ``kit/src/widgets/view/distribution/tests.rs::the_title_legend_cuts_the_names_and_keeps_the_unit``
   -- the names are cut before the unit at the end.
   ``kit/src/widgets/view/stats/tests.rs::the_hover_of_a_stat_has_all_its_digits``
   -- the hover of a statistic has all its digits. The unit left out for entries
   that fit, and the legend and tooltip of a Plot showing one sample, are
   checked by hand.
