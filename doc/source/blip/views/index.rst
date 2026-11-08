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
text (plot, states, table, distribution, heatmap) also have a **Number format** section:
for each trace how its numbers are written -- decimal, hex, binary or
scientific -- next to its unit as sent by the target. Hex and binary apply to
whole numbers only; strings and bools have no format.

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
