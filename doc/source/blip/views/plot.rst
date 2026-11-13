Plot
====

.. figure:: /screenshots/view-plot.png
   :target: ../../_images/view-plot.png
   :class: screenshot
   :alt: A plot of the robot's position and heading over 20 s, with the time cursor.

A time-series plot of the checked number and bool traces. Numbers are drawn
as lines and bools as steps between 0 and 1, both with gaps where a ``null``
was received.

.. figure:: /screenshots/view-plot-tooltip.png
   :target: ../../_images/view-plot-tooltip.png
   :class: screenshot
   :alt: The tooltip of a plot, with the time and a row per trace with its value.

Hovering the plot shows a tooltip with the time and a row per shown trace: its
dot and name, and its value at the pointer with the unit. A trace without a
value there shows a faint *—*. A value with a label shows the label and the
number faint beside it. Below a rule, the tooltip gives the name and
description of the curve nearest the pointer, if it is within a few pixels of
it and its target sent a description.

* **Drag sideways** to move the time range (this pauses), and **scroll the
  mouse wheel** to zoom it, which changes the window length. See
  :doc:`/blip/time`.
* The y axis fits all data of its traces, also outside the shown range,
  unless its min or max is set. When a trace says what range its values are
  expected in, the axis fits that range instead, see **Fit to** below. **Drag up or down** to pan it, scroll with
  :kbd:`Alt` held to zoom it, and **double-click** to fit it again.
* **Click** to put the time cursor there, or drag the cursor line.

Properties:

.. list-table::
   :widths: 25 75

   * - **Y axis**
     - **Fit to**, only while a shown trace declares its expected range:
       *Expected* fits the declared ranges and the values of the traces that
       declare none, and values beyond a declared range grow the axis;
       *Values* fits the values of the traces, as for traces without a range.
       **Min** and **Max**, empty to fit, show the ends in use as faint
       hints; **Label**, by default the unit the traces share.
   * - **Lines**
     - **Interpolation**: *Linear* or *Step*. **Width** of the lines.
       **Points**: mark the samples *Auto* (when they are far enough apart),
       *Always* or *Never*. **Nulls**: leave a *Gap* or *Connect* across
       them.
   * - **Legend**
     - **Placement**: *Title*, in the title row with the value of each trace
       at the time cursor, in a corner, or hidden. A plot alone in a tab has
       no title row and shows the title legend at the top right.
   * - **Line styles**
     - Per trace *Solid*, *Dashed*, *Dotted* or *None* (points only).
   * - **Stats**
     - Measurements shown below the plot, like those of an oscilloscope, one
       per row: a trace and a stat, added with **Add stat** and removed with
       **−**. *Min*, *Max*, *Pk-Pk*, *Mean*, *RMS* and *Std dev* are taken
       over the shown time range, each value held until the next sample.
       *Frequency*, *Period* and *Duty* come from where the trace crosses its
       mid level, halfway between its min and max, ignoring noise within 10 %
       of its range around that level; they show *—* without a full cycle.
       Stats of traces no longer shown are kept, greyed, for when the trace is
       shown again.
