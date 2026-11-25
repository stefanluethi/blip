XY and Heatmap
==============

Tests of the requirements in :doc:`/requirements/system/xy-heatmap`.

.. test:: XY axes, curves and legend
   :id: TEST_BLIP_XY_PROPS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_XY_PROPS

   ``kit/src/widgets/view/xy/tests.rs`` --
   ``a_curve_takes_the_x_trace_chosen_or_swaps_with_the_trace_chosen`` and
   ``a_y_trace_chosen_swaps_with_the_y_trace_of_the_curve`` check the choices of
   the X and the Y trace of a curve in the section Axes;
   ``an_axis_is_set_only_by_ends_in_order``;
   ``the_legend_goes_to_a_corner_without_a_title_row`` that the legend is in the
   title row or in a corner as chosen; ``xy_plots_saved_before_the_new_properties_load_to_the_same_picture``
   that older XY plots load to the same picture. The sections X axis and Y axis
   with their Label and the absence of hover rulers are checked by hand.

.. test:: Heatmap at the cursor and its grid
   :id: TEST_BLIP_HEATMAP_PROPS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_HEATMAP_PROPS

   ``kit/src/widgets/view/heatmap/tests.rs`` --
   ``the_y_axis_at_the_cursor_stays_where_zoomed_until_a_double_click``;
   ``a_grid_fills_its_frame_with_square_cells_only_while_they_fill_half_of_it``
   (4 × 4 cells stretch in a wide frame, stay square in a frame that they fill
   two thirds of, and a small array in a tall card fills it);
   ``heatmaps_saved_with_a_fixed_or_no_range_load_to_the_same_scale`` that the
   line at the cursor keeps its points, width and styles as before.
