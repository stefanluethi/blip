Trace metadata
==============

Tests of the requirements in :doc:`/requirements/system/metadata`.

.. test:: Labels replace integers
   :id: TEST_BLIP_LABELS
   :status: implemented
   :component: blip
   :verifies: REQ_META_LABELS

   ``blip/src/app/model/numbers`` unit tests ``numbers_get_their_labels``,
   ``labels_replace_numbers_only_with_the_labels_format`` and
   ``labels_are_offered_first_for_labelled_numbers`` check that an integer or a
   whole float gets its label, a fraction, a bool, null and an integer without a
   label do not, that only the format Labels writes labels and the other formats
   show the numbers, and that Labels is offered first for integers and floats
   with labels and never for bools. ``blip/src/app/view`` unit test
   ``labels_are_the_default_of_labelled_traces`` checks that Labels is the
   default of a trace with labels, and ``blip/src/app/view/states``
   ``bands_write_labels_and_keep_the_colour_of_the_number`` that the bands write
   labels while their colour follows the number, a label that is a log level name
   taking the level's colour. ``blip/src/app/view/table``
   ``labelled_cells_write_the_label_until_the_default_is_chosen`` and
   ``blip/src/app/view/plot`` ``labels_show_in_the_title_legend`` check the cells
   and the legend value at the cursor. ``blip/src/app/view/tooltip`` unit test
   ``labelled_values_have_their_number_faint`` checks that a label is followed by
   the faint number in Default with its unit, and that a number without a label or
   in another format is written as a number, and ``blip/src/app/view/states``
   ``the_tooltip_describes_the_row_and_writes_labels_with_the_number`` and
   ``blip/src/app/view/xy`` ``the_tooltips_of_points_and_elements_describe_the_trace``
   that the States and XY tooltips write labels.

.. test:: The properties list the facts of the traces
   :id: TEST_BLIP_META_PROPS
   :status: implemented
   :component: blip
   :verifies: REQ_META_PROPS

   ``blip/src/app/view`` unit tests ``the_facts_of_a_trace_are_written_in_one_line_each``
   (``Expected <min> to <max>``, the labels on one line and one per line for the
   hover) and ``the_traces_section_lists_the_facts`` (kittest: the section Traces
   shows the description, the expected range and the labels, no section Number
   format, and a trace without facts adds no row), and ``blip/src/app/view/table``
   ``columns_are_hidden_and_formatted_in_the_properties``, which finds the
   ``Expected`` line under a column of the Records layout.

.. test:: Tooltips show raw numbers and a description footer
   :id: TEST_BLIP_META_TOOLTIP
   :status: implemented
   :component: blip
   :verifies: REQ_META_TOOLTIP

   ``blip/src/app/view/tooltip`` unit tests ``labelled_values_have_their_number_faint``
   and ``the_footer_has_the_name_and_description_in_at_most_3_lines`` check that a
   labelled value has a faint raw number next to it, that the footer is set only
   with a description, wraps in at most 3 rows ending with an ellipsis, and shows
   below the rows in an ``egui_kittest`` tooltip. ``blip/src/app/view/states``
   ``the_row_under_the_pointer_is_found_by_its_height`` and
   ``the_tooltip_describes_the_row_and_writes_labels_with_the_number``,
   ``blip/src/app/view/plot`` ``the_curve_nearest_the_pointer_within_12_points_is_the_one_under_it``
   and ``the_tooltip_describes_the_trace_near_the_pointer``,
   ``blip/src/app/view/heatmap`` ``the_tooltips_of_a_heatmap_describe_its_trace``,
   ``blip/src/app/view/xy`` ``the_tooltips_of_points_and_elements_describe_the_trace``
   (the elements only with one array), ``blip/src/app/view/distribution/boxes``
   ``the_tooltip_of_a_box_describes_the_trace`` and ``blip/src/app/view/distribution``
   ``the_bin_tooltip_describes_the_trace_only_with_one`` check the trace the footer
   names in each view. The screenshot ``view-states-labels-tooltip`` shows it.

.. test:: Descriptions show on hover in trees and tables
   :id: TEST_BLIP_META_HOVER
   :status: implemented
   :component: blip
   :verifies: REQ_META_HOVER

   ``blip/src/app/view`` unit test ``the_hover_of_a_trace_has_its_path_type_and_description``
   checks ``trace_hover`` with and without a description, which the tree, the
   name of a trace in a table and the header of a column in the Records layout
   all use; ``blip/src/app/view/trace_selection``
   ``the_tree_row_of_a_described_trace_has_its_description_on_hover`` hovers a row
   of the tree and finds the description.

.. test:: The plot axis fits the expected range
   :id: TEST_BLIP_FIT_PLOT
   :status: implemented
   :component: blip
   :verifies: REQ_META_FIT_PLOT

   ``blip/src/app/view/plot`` unit tests
   ``fit_to_is_offered_only_while_a_shown_trace_declares_a_range``,
   ``expected_fits_the_declared_ranges_grown_by_the_values``,
   ``a_typed_end_overrides_the_fit_and_clearing_it_returns`` and
   ``the_hint_of_an_end_is_the_end_in_use`` check that the row Fit to is shown
   only while a shown trace declares a range; that Expected fits the union of
   the ranges and the values of the traces without one and grows with values
   beyond; that Values fits the values; that the fields show the ends in use
   as hints, typing overrides one end and clearing returns to the fit; and
   that the range of ``shown_y``, which a linked Distribution takes, is the
   resulting one.

.. test:: The XY axes fit the expected ranges
   :id: TEST_BLIP_FIT_XY
   :status: implemented
   :component: blip
   :verifies: REQ_META_FIT_XY

   ``blip/src/app/view/xy`` unit test
   ``the_axes_fit_the_declared_ranges_with_expected`` checks that Expected
   takes the X axis from the declared range of the X trace and the values, and
   the Y axis from the union of the declared ranges of the Y traces and all
   values, and that Values fits the values only.

.. test:: The heatmap scale starts from the expected range
   :id: TEST_BLIP_HEATMAP_RANGE
   :status: implemented
   :component: blip
   :verifies: REQ_META_HEATMAP

   ``blip/src/app/view/heatmap`` unit tests
   ``the_colour_scale_starts_from_the_expected_range`` and
   ``a_fixed_range_starts_with_the_one_shown`` check that Auto is the declared
   range of the array trace, the fitted values without one, and that Fixed
   starts with that range in its min and max.

.. test:: Metadata settings are saved with the layout
   :id: TEST_BLIP_META_SAVED
   :status: implemented
   :component: blip
   :verifies: REQ_META_SAVED

   ``blip/src/app/view`` unit test ``labels_are_the_default_of_labelled_traces``
   checks that the choice of Default for a labelled trace is saved as
   ``"Default"`` and the default Labels as nothing, and
   ``blip/src/app/presets`` unit test
   ``presets_are_written_as_version_2_and_version_1_is_read`` that presets are
   written as version 2, a version 1 file still reads and a version 3 file is
   refused, and that a preset with a Plot of Fit to Values and a Labels format
   is written and read back unchanged. ``blip/src/app/view`` unit test
   ``fit_to_and_formats_survive_json`` checks that the Fit to choices of a Plot
   and an XY and the formats survive JSON, and that a view saved before loads
   with Expected; ``the_demo_plots_fit_to_the_expected_range`` in presets that
   the demo presets of version 1 read without a Fit to.

.. test:: Screenshots of the trace facts and the connection row
   :id: TEST_BLIP_META_SHOTS
   :status: implemented
   :component: blip, doc
   :verifies: REQ_META_PROPS, REQ_CONN_HELLO_SHOW

   ``blip/src/app/screenshots.rs`` renders ``panel-properties-traces``, the
   Traces section of the properties, and ``view-states-labels-tooltip``, a
   tooltip with labels and a description footer, for the documentation.
   ``screenshots_connections`` renders ``popup-connections``, the connections
   popup with two ``Hello`` rows (one with a newer protocol) and an unconnected one.
