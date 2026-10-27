Views
=====

A view shows the traces checked for it in the traces panel (see
:doc:`traces`), or the logs, or a form of actions. Add views in an empty tab
or by right-clicking the title of a view, see :doc:`layout`; the settings of
the selected view are in the properties panel on the right and are saved with
the layout.

.. list-table::
   :header-rows: 1
   :widths: 15 40 45

   * - View
     - Shows
     - Takes
   * - Plot
     - values over time, as lines
     - numbers and bools, elements of arrays of up to 16 elements
   * - States
     - values over time, as coloured bands
     - strings, bools and integers, elements of such arrays
   * - Table
     - the values at the time cursor
     - any trace, or whole records
   * - XY
     - traces against another trace, or arrays over their indices
     - numbers and bools; one-dimensional arrays
   * - Heatmap
     - an array as colours, over time or at the cursor
     - one array of one or two dimensions
   * - Logs
     - the log records
     - --
   * - Actions
     - a form to carry out actions
     - actions, see :doc:`actions`

Every view's properties start with its **Name**. Views that write values as
text (plot, states, table, heatmap) also have a **Number format** section:
for each trace how its numbers are written -- decimal, hex, binary or
scientific -- next to its unit as sent by the target. Hex and binary apply to
whole numbers only; strings and bools have no format.

Number fields in the properties that are left empty show *auto*: the value is
fitted to the data.

Plot
----

.. figure:: /screenshots/view-plot.png
   :target: ../../_images/view-plot.png
   :class: screenshot
   :alt: A plot of the robot's position and heading over 20 s, with the time cursor.

A time-series plot of the checked number and bool traces. Numbers are drawn
as lines and bools as steps between 0 and 1, both with gaps where a ``null``
was received. Hovering the plot shows the values of its traces at the
pointer.

* **Drag sideways** to move the time range (this pauses), and **scroll the
  mouse wheel** to zoom it, which changes the window length. See
  :doc:`time`.
* The y axis fits all data of its traces, also outside the shown range,
  unless its min or max is set. **Drag up or down** to pan it, scroll with
  :kbd:`Alt` held to zoom it, and **double-click** to fit it again.
* **Click** to put the time cursor there, or drag the cursor line.

Properties:

.. list-table::
   :widths: 25 75

   * - **Y axis**
     - **Min** and **Max**, empty to fit the data; **Label**, by default the
       unit the traces share.
   * - **Lines**
     - **Interpolation**: *Linear* or *Step*. **Width** of the lines.
       **Points**: mark the samples *Auto* (when they are far enough apart),
       *Always* or *Never*. **Nulls**: leave a *Gap* or *Connect* across
       them.
   * - **Legend**
     - **Placement**: hidden or in a corner.
   * - **Line styles**
     - Per trace *Solid*, *Dashed*, *Dotted* or *None* (points only).

States
------

.. figure:: /screenshots/view-states.png
   :target: ../../_images/view-states.png
   :class: screenshot
   :alt: States of the motor and the log level as coloured bands.

The values of strings, bools and integers over time, as a row of coloured
bands per trace, one band for each run of equal values, e.g. the state of a
state machine. Runs too short to tell apart are drawn as one band showing
that the value changes too often. Hover a band for its value and time; click
to put the time cursor there.

Floats cannot be shown as states; plot them instead.

Properties: **Bands** -- **Height** of a band as a fraction of its row, and
**Values** to label the bands wide enough with their value.

Table
-----

.. figure:: /screenshots/view-table.png
   :target: ../../_images/view-table.png
   :class: screenshot
   :alt: A table of values at the time cursor, with units and arrays.

The values of the checked traces at the time cursor, i.e. the latest sample
at or before it. Arrays show a few elements per dimension; hover them for
more.

Properties, **Rows**:

* **Layout** -- *Traces*: a row per trace. *Records*: a row per record and a
  column per field, e.g. ``acc.z``; the traces panel then checks whole
  records, and a checked branch adds the records that arrive below it later.
* **Traces** (*Traces* layout) -- list them as a *Tree*, by their full
  *Paths*, or by their *Names* only.
* **Empty** (*Traces* layout) -- also show traces without a value at the
  cursor time.

With the *Records* layout, the **Columns** section shows and hides the fields,
reorders them by dragging, and sets the number format of each. Fields that
arrive later are shown.

XY
--

.. figure:: /screenshots/view-xy.png
   :target: ../../_images/view-xy.png
   :class: screenshot
   :alt: An XY view of the robot's figure eight, the trail fading out, with the marker at the cursor.

Traces plotted against one of them instead of against time, e.g. a position
as a path: the samples within the shown window, as a trail that fades towards
older points, with a marker where each trace is at the time cursor. Each
sample of a y trace is paired with the value the x trace had at its time.

Checking one-dimensional arrays instead plots each array at the cursor time
over the indices of its elements, e.g. a snapshot of ADC samples or a
spectrum.

The view fits all data of its traces until it is dragged or zoomed.

Properties:

* **Axes** -- for each trace whether it is on the **X** or the **Y** axis
  (one trace is on the x axis); **Equal axes** gives a unit the same length on
  both axes, e.g. for positions.
* **Lines** -- **Points** as for plots, and **Fade**: how far the oldest part of
  the trail fades out.
* **Line styles** -- as for plots.

Heatmap
-------

.. figure:: /screenshots/view-heatmap-grid.png
   :target: ../../_images/view-heatmap-grid.png
   :class: screenshot
   :alt: An 8×8 thermal camera frame as a heatmap grid with a colour bar.

One array trace of one or two dimensions as coloured cells, with a colour
bar of the scale.

* A **1D array** is shown **over time** by default, each element a row of
  colours, like a spectrogram of a spectrum; or **at the cursor** as a line
  over the indices of its elements.
* A **2D array** is shown as a grid at the cursor time, e.g. a frame of a
  thermal camera. Cells wide enough show their value.

Properties: **Array** -- **Show** *Over time* or *At cursor* (1D arrays), and
**Colour scale** -- fitted to the shown values, or fixed between a min and a
max.

Logs
----

.. figure:: /screenshots/view-logs.png
   :target: ../../_images/view-logs.png
   :class: screenshot
   :alt: A log view with times, coloured levels, components and messages.

A table of the received log records with time, level, component and message.
Records outside the shown time range are greyed out. The filters are in the
column headers and are kept with the view:

* **Level** and **Component** -- checkboxes for what is shown, with **All**
  and **None**. New components are shown.
* **Message** -- a search, as a regular expression; **Aa** matches case. Text
  that is no valid regular expression is searched as it is.

Below the table, the arrow button follows the newest records, the count says
how many of all records are shown, and **Clear filters** shows all again.

Actions
-------

A form of the checked actions, to carry them out on their targets. See
:doc:`actions`.
