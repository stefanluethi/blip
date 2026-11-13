Views
=====

A view shows the traces checked for it in the traces panel (see
:doc:`/blip/traces`), or the logs, or a form of actions. Add views in an empty tab,
with **⋯** on the title of a view or with **View › Add view**, see
:doc:`/blip/layout`; the settings of the selected view are in the properties panel
on the right and are saved with the layout.

.. list-table::
   :header-rows: 1
   :widths: 15 40 45

   * - View
     - Shows
     - Takes
   * - :doc:`plot`
     - values over time, as lines
     - numbers and bools, elements of arrays of up to 16 elements
   * - :doc:`states`
     - values over time, as coloured bands
     - strings, bools and integers, elements of such arrays
   * - :doc:`table`
     - the values at the time cursor
     - any trace, or whole records
   * - :doc:`xy`
     - traces against other traces, or arrays over their indices
     - numbers and bools; one-dimensional arrays
   * - :doc:`distribution`
     - how the values spread over the shown time range, as bars or box plots
     - numbers, elements of arrays of up to 16 elements
   * - :doc:`heatmap`
     - an array as colours, over time or at the cursor
     - one array of one or two dimensions
   * - :doc:`logs`
     - the log records
     - --
   * - Actions
     - a form to carry out actions
     - actions, see :doc:`/blip/actions`

Every view's properties start with its **Name**. Views that write values as
text (plot, states, table, distribution, heatmap) also have a **Traces** section.
For each trace it shows how its numbers are written -- decimal, hex, binary or
scientific, or as labels -- next to its unit, and what the target says of the
trace: its description, the range its values are *Expected* in (*Expected 0 to
3000*) and its labels (*0 Idle, 1 Ramp, 2 Run*, cut at the end; hover for all
of them). Hex and binary apply to whole numbers only; strings and bools have no
format. A line the target did not send is left out.

.. figure:: /screenshots/panel-properties-traces.png
   :target: ../../_images/panel-properties-traces.png
   :class: screenshot
   :width: 300px
   :alt: The properties of a plot with the section Traces: for each trace its number format and unit, its description, its expected range and its labels.

   The properties of a plot of two traces that tell what they are.

The format **Labels** is offered first for a trace that has labels, and is its
default: the label replaces the number in the bands of States, the cells of a
table, the tooltips and the value of the legend. A number without a label stays
a number. Choose another format to see the numbers.

Number fields in the properties that are left empty show *auto*: the value is
fitted to the data.

Actions
-------

A form of the checked actions, to carry them out on their targets. See
:doc:`/blip/actions`.

Next steps
----------

* :doc:`/blip/layout` -- add, arrange and save views.
* :doc:`/blip/time` -- choose the time the views show.

.. toctree::
   :hidden:

   plot
   states
   table
   xy
   distribution
   heatmap
   logs
