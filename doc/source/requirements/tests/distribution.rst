Distribution
============

Tests of the requirements in :doc:`/requirements/system/distribution`.

.. test:: Distribution box statistics
   :id: TEST_BLIP_DIST_STATS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_DIST_STATS, REQ_VIS_DIST_WHISKERS, REQ_VIS_DIST_BOX_EMPTY

   ``blip/src/app/view/distribution/boxes.rs::quartiles_interpolate_between_the_samples_as_numpy``
   -- quartiles and percentiles of known samples are those of numpy's
   default (type 7), linear between the order statistics.
   ``blip/src/app/view/distribution/boxes.rs::whiskers_end_at_the_fences_the_percentiles_or_the_extremes``
   -- the whiskers of 1.5×IQR end at the furthest samples within the fences,
   those of 5–95 % and 1–99 % at the percentiles and Min–max at the
   extremes; the samples beyond them are the outliers, none with Min–max.
   ``blip/src/app/view/distribution/boxes.rs::nulls_infinite_samples_and_equal_values``
   -- nulls and infinite samples are not in the box, no samples give no box,
   equal values give a box of the median alone with the mean on it, with
   every whisker choice.
   ``blip/src/app/view/distribution/boxes.rs::a_box_wholly_beyond_the_value_axis_points_to_its_edge``
   -- a box wholly above or below the value axis gets an arrow at that edge,
   one partly shown is cut at the edge instead.
   ``blip/src/app/view/distribution.rs::boxes_count_each_sample_once_over_the_shown_window``
   -- a box counts the samples of the shown window without nulls, anew as
   the window pans, zooms and moves on while live; a sample that holds long
   counts once; a fixed range or a linked plot's range does not change it.
   ``blip/src/app/view/distribution.rs::a_trace_without_samples_keeps_its_row``
   -- a trace without samples in the window has no box.

.. test:: Distribution forms, boxes and properties
   :id: TEST_BLIP_DIST_BOXES
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_DIST_SHOW, REQ_VIS_DIST_BOX, REQ_VIS_DIST_BOX_EMPTY, REQ_VIS_DIST_PROPS, REQ_VIS_HIST_ORIENT, REQ_VIS_HIST_LINK

   ``blip/src/app/view/distribution.rs::the_form_is_bars_by_default_and_boxes_give_each_trace_a_row``
   (``egui_kittest``) -- the form is Bars by default; Boxes gives each trace
   an equal row covering the plot, from the top upright and from the left
   sideways, named by the traces, without counts.
   ``blip/src/app/view/distribution.rs::the_names_of_the_rows_are_cut_at_a_third_of_the_view``
   (``egui_kittest``) -- long names are cut with "…" to a third of the view,
   and sideways to their columns.
   ``blip/src/app/view/distribution.rs::both_puts_a_box_per_trace_in_the_strip_of_the_markers``
   (``egui_kittest``) -- Both puts a 10 pt box per trace in 14 pt rows of the
   strip at the end of the counts axis, in place of the markers, over the
   bars.
   ``blip/src/app/view/distribution/boxes.rs::a_box_is_40_percent_of_its_row_at_most_24_pt``
   -- a box is 40 % of its row thick, at least 6 and at most 24 pt, with caps
   half as long; the rows and the strip of Both lie where they should, and
   the row under a point is found.
   ``blip/src/app/view/distribution.rs::a_trace_without_samples_keeps_its_row``
   (``egui_kittest``) -- in Boxes and Both a trace without samples keeps its
   row, whose tooltip says "No samples".
   ``blip/src/app/view/distribution.rs::the_properties_hide_the_rows_that_do_not_apply_to_the_form``
   (``egui_kittest``) -- Bars shows Markers, Bins and Counts; Boxes shows
   Whiskers in the place of Markers and hides Bins and Counts; Both shows
   Whiskers, Bins and Counts; Range is in all; the whiskers are chosen in a
   combo box and the hidden settings are kept.
   ``blip/src/app/view/distribution.rs::a_linked_view_follows_the_plot_in_all_forms``
   (``egui_kittest``) -- in Bars, Boxes and Both, upright and sideways, a
   linked view shows the plot's y range as it changes and the wheel does not
   zoom it.

.. test:: Distribution box hover
   :id: TEST_BLIP_DIST_HOVER
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_DIST_BOX_HOVER

   ``blip/src/app/view/distribution.rs::hovering_a_row_lights_it_and_shows_the_box_of_the_trace``
   (``egui_kittest``) -- hovering anywhere in a row of Boxes, also away from
   the box, makes it the hovered row, lit, and shows the name of the trace and
   min, low whisker, Q1, median, mean, Q3, high whisker and max, from the top
   down upright and from the bottom up sideways, right aligned in the number
   format (Scientific here) and unit of the trace, then under them Samples and
   Outliers, the count right aligned with the values.
   ``blip/src/app/view/distribution.rs::min_max_whiskers_leave_out_their_rows_and_percentiles_are_named``
   (``egui_kittest``) -- with Min–max the whisker and outlier rows are left
   out, with 5–95 % and 1–99 % the whiskers are named "5 %", "95 %", "1 %"
   and "99 %".
   ``blip/src/app/view/distribution.rs::hovering_the_strip_of_both_shows_a_box_and_below_it_a_bin``
   (``egui_kittest``) -- in Both, hovering a row of the strip shows the box of
   its trace, below the strip a bin.
   ``blip/src/app/view/distribution/boxes.rs::the_tooltip_lists_the_values_in_the_order_drawn``
   -- the rows of the tooltip in the order drawn, the box in the text colour
   and its ends weak, the values written to a thousandth of the range.

.. test:: Distribution screenshots
   :id: TEST_BLIP_DIST_SHOTS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_DIST_SHOW, REQ_VIS_DIST_BOX

   ``blip/src/app/screenshots.rs::screenshots_views`` renders
   ``view-distribution-boxes``, the upright boxes of ``robot/pose.x``,
   ``robot/pose.y`` and ``robot/pose.heading``, and
   ``view-distribution-both``, the bars of ``robot/pose.x`` and
   ``robot/pose.y`` with their boxes in the strip;
   ``blip/src/app/screenshots.rs::screenshots_layouts`` renders
   ``view-distribution-boxes-sideways``, the sideways box of
   ``motor.speed_rpm`` beside the plot it is linked to; and
   ``blip/src/app/screenshots.rs::screenshots_panels`` renders
   ``panel-properties-distribution``, the properties of that view; from the
   demo recording in the light and the dark theme, for the documentation and
   to check by eye that they look right.
