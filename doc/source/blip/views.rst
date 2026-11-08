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
     - traces against other traces, or arrays over their indices
     - numbers and bools; one-dimensional arrays
   * - Distribution
     - how the values spread over the shown time range, as bars or box plots
     - numbers, elements of arrays of up to 16 elements
   * - Heatmap
     - an array as colours, over time or at the cursor
     - one array of one or two dimensions
   * - Logs
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

Plot
----

.. figure:: /screenshots/view-plot.png
   :target: ../_images/view-plot.png
   :class: screenshot
   :alt: A plot of the robot's position and heading over 20 s, with the time cursor.

A time-series plot of the checked number and bool traces. Numbers are drawn
as lines and bools as steps between 0 and 1, both with gaps where a ``null``
was received. Hovering the plot shows the values of its traces at the
pointer.

* **Drag sideways** to move the time range (this pauses), and **scroll the
  mouse wheel** to zoom it, which changes the window length. See
  :doc:`/blip/time`.
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

States
------

.. figure:: /screenshots/view-states.png
   :target: ../_images/view-states.png
   :class: screenshot
   :alt: States of the motor and the log level as coloured bands.

The values of strings, bools and integers over time, as a row of coloured
bands per trace, one band for each run of equal values, e.g. the state of a
state machine. Runs too short to tell apart are drawn as one band showing
that the value changes too often. Hover a band for its value and time; click
to put the time cursor there.

Each value keeps its colour: a light band under an edge in the full colour.
A ``false`` bool leaves its band empty, so ``true`` reads as on, and log
levels such as ``DEBUG`` or ``ERROR`` take the colours of the log view.

Floats cannot be shown as states; plot them instead.

Properties: **Bands** -- **Height** of a band as a fraction of its row, and
**Values** to label the bands wide enough with their value.

Table
-----

.. figure:: /screenshots/view-table.png
   :target: ../_images/view-table.png
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
   :target: ../_images/view-xy.png
   :class: screenshot
   :alt: An XY view of the robot's figure eight, the trail fading out, with the marker at the cursor.

Traces plotted against other traces instead of against time, e.g. positions
as paths: the samples within the shown window, as a trail that fades towards
older points, with a marker where each trace is at the time cursor. Each y
trace is plotted against one x trace, and each of its samples is paired with
the value that x trace had at its time.

Checked traces named ``x`` and ``y`` with the same parent, like the fields of
a record ``robot/pose``, pair up: ``y`` is plotted against ``x``, and the line
is named after the parent, whichever of them is checked first, unless other
traces are plotted against ``y``. Other checked traces are plotted against the
first x trace; the first trace checked is the x trace if there is none.

Checking one-dimensional arrays instead plots each array at the cursor time
over the indices of its elements, e.g. a snapshot of ADC samples or a
spectrum. The axes of the other traces are kept for when no array is checked.

The view fits all data of its traces until it is dragged or zoomed.

Properties:

* **Axes** -- for each trace whether it is on the **X** or the **Y** axis, and
  for a y trace the x trace it is plotted against (any number of traces can be
  on the x axis, e.g. the positions of two robots); **Equal axes** gives a unit
  the same length on both axes, e.g. for positions.
* **Lines** -- **Points** as for plots, and **Fade**: how far the oldest part of
  the trail fades out.
* **Line styles** -- as for plots.

Distribution
------------

.. figure:: /screenshots/view-distribution.png
   :target: ../_images/view-distribution.png
   :class: screenshot
   :alt: A distribution of the robot's x and y position as bars drawn over each other, with the mean and σ of each as whiskers above them and the legend at the top right.

How the values spread over the shown time range, as bars, as box plots or as
both. It takes integer and float traces and the elements of arrays of up to 16
elements; ``null`` samples are not counted. It counts anew as the range is
moved, zoomed or moves on while live. Choose the form in **Show**:

* **Bars** (the default): how often each value occurs. The samples of the
  checked traces are counted in bins of their values, drawn as bars.
* **Boxes**: a box plot, a row per trace, see below.
* **Both**: the bars, and a box per trace in the strip above them in place of
  the markers.

The traces are counted in the same bins and drawn over each other, each as a
light area under an outline in its colour. The legend shows a dot and the name
of each trace in the title row. Without a title row it is at the top right of
the plot, as in a plot view; there it may cover the end of the marker strip or
a tall bar. The value axis, the bin edges and the legend in the title row show
the unit only if all traces share one.

Samples that lie outside the bins are not binned but counted, and their counts
shown faintly at the end they lie beyond, e.g. *‹ 232* and *36 ›* (*↓* and *↑*
sideways): those outside a fixed range or the range of a linked plot, and
infinite samples, whatever the range.

* **Hover** a bin to highlight it and read its range of values and, for each
  trace, its percentage and count of the samples, e.g. *12.5 %* and *42 of
  336*.
* **Scroll the mouse wheel** to zoom the value axis, and **drag** to pan it;
  the bins are magnified, not counted anew. **Double-click** to fit it again.
  None of them changes the time range or the time cursor.
* Without checked traces it asks to check them in the tree; without samples
  in the shown range it shows *No samples in the shown window*; with only
  infinite samples it shows their counts at the ends of empty axes. If all
  samples have the same value, it shows one bar centred on it.

The counts axis shows the samples of each trace in a bin as a percentage of
all samples of that trace in the shown range, those outside the bins included,
so traces of different sample rates compare. It fits the tallest bin in view: it
grows at once, and shrinks only once that bin is well below its top, so it
does not jump while live. The bars leave room at its far end for the markers
and the counts outside the bins.

Boxes
~~~~~

.. figure:: /screenshots/view-distribution-boxes.png
   :target: ../_images/view-distribution-boxes.png
   :class: screenshot
   :alt: A box plot of the robot's x and y position, a row per trace with its name on the left, each a box with a median line, a mean dot, whiskers and outlier dots.

With **Show** set to *Boxes* each trace has a row with its name on the
category axis; long names are cut with *…* at a third of the view. A box
spans the first to the third quartile as a light area under an outline, with a
line at the median, a dot at the mean, whiskers with caps, and a dot for each
outlier beyond the whiskers. Sideways, the traces are columns with their names
below.

.. figure:: /screenshots/view-distribution-both.png
   :target: ../_images/view-distribution-both.png
   :class: screenshot
   :alt: The bars of the robot's x and y position with a box for each trace in the strip above them.

With *Both* the boxes are drawn in the strip above the bars (to the right of
them sideways), in rows of 14 pt, and replace the mean and σ markers.

.. figure:: /screenshots/view-distribution-boxes-sideways.png
   :target: ../_images/view-distribution-boxes-sideways.png
   :class: screenshot
   :alt: A sideways box plot of the robot's x and y position, a column per trace with its name below.

The **Whiskers** end at:

* *1.5×IQR* (the default): the furthest sample within 1.5 times the length of
  the box from it. Samples beyond are outliers.
* *Min–max*: the lowest and the highest sample. There are no outliers.
* *5–95 %* or *1–99 %*: those percentiles. Samples beyond are outliers.

The statistics count all samples that are not ``null`` and finite in the shown
time range, each once, however long it lasts, unlike the stats of a plot. The
quartiles and percentiles are interpolated between samples. Infinite samples
are left out of the boxes, while the bars still count them at their ends. The
boxes do not depend on the **Range**: a fixed or linked range only cuts them at
its edge, and a box wholly beyond it shows a faint arrow at that edge (*‹* and
*›*, *↓* and *↑* sideways).

**Hover** a row to light it and read, in the order drawn (low to high; high to
low sideways), its min, low whisker, Q1, median, mean, Q3, high whisker and
max, then the number of samples and of outliers. With *Min–max* the whisker
and outlier rows are left out, and percentile whiskers are named by their
percentile, e.g. *5 %* and *95 %*. A trace without samples in the range keeps
its row and shows *No samples*.

Properties:

.. figure:: /screenshots/panel-properties-distribution.png
   :target: ../_images/panel-properties-distribution.png
   :class: screenshot
   :alt: The properties panel of a distribution with Link, Display, Values, Bins and Counts.

Rows that do not apply to the chosen form are hidden. The settings of a hidden
row are kept.

.. list-table::
   :widths: 25 75

   * - **Link**
     - **Plot**: a plot of the layout whose traces and y range the
       distribution shows, upright or sideways, or *None*; see below.
   * - **Display**
     - **Show**: *Bars*, *Boxes* or *Both*. **Orientation**: *Upright*, the
       values along the bottom and the bars growing up, or *Sideways*, the
       values up the side and the bars growing to the right, e.g. beside a
       plot of the same traces. **Axis side** of a sideways distribution: the
       value axis on the *Left*, as in a plot, or on the *Right*.
       **Markers** (*Bars*): *Off*, *Mean*, or *Mean ±σ* for all traces, in a
       strip above the bars (to the right of them sideways), a row per
       trace: a dot at the mean and a whisker from the mean −σ to the mean
       +σ. Unlike the stats of a plot, each sample counts once, however long
       it lasts; σ is the population standard deviation. **Whiskers**
       (*Boxes* and *Both*): where the whiskers end, see above.
   * - **Values**
     - **Range**: *Auto* fits the values shown, with 5 % room on each side
       for the bars; *Fixed* shows only those between **Min** and **Max**,
       and the bars count the others at the ends.
   * - **Bins** (*Bars* and *Both*)
     - **Mode**: *Auto*, about the square root of the number of samples, at
       least 8 and at most 64, of a round width (1, 2 or 5 times a power of
       ten) that changes only once the ideal width is clearly past a step, so
       it does not flip between two widths while live; *Number*, a fixed
       number of bins split evenly over the range of the values; or *Width*,
       bins of a fixed width in the unit of the traces. Automatic and
       fixed-width bins start at multiples of their width, so the bars stay
       put as the range moves. Over integer traces, bins of a whole-number
       width start half a unit below those multiples instead, so no integer
       lies on an edge and each bin holds the same number of integers; with a
       width of 1, the bar of 5 runs from 4.5 to 5.5. Switching to *Number* or
       *Width* starts from the bins shown.
   * - **Counts** (*Bars* and *Both*)
     - **Scale**: *Linear* or *Log*. The logarithmic scale has a tick at each
       power of ten, e.g. *0.1 %*, *1 %*, *10 %*, and starts at least half a
       decade below the percentage of one sample, so a bin of one sample shows
       as a short bar and an empty bin shows nothing.

Link to a plot
~~~~~~~~~~~~~~

.. figure:: /screenshots/view-distribution-sideways.png
   :target: ../_images/view-distribution-sideways.png
   :class: screenshot
   :alt: A plot of the motor speed and, beside it, a sideways distribution linked to the plot, so its bars line up with the heights of the line.

A distribution linked to a plot shows the spread of the plot's traces over the
plot's y range; sideways beside the plot, at the heights of the plot. Choose
the plot in **Link** › **Plot**, upright or sideways:

* The distribution shows the traces the plot shows, with their number formats,
  and follows as traces are checked in the plot. With the distribution
  selected, the tree shows them checked but greyed, under *Shows the traces
  of the linked plot …*; check them in the plot. Its **Number format** is
  greyed too (*Set by the linked plot*).
* The value axis shows the y range the plot shows, as it is zoomed, panned
  or fixed. The bins cover that range, and the samples outside it are
  counted at its ends, as for a fixed range; boxes are cut at it. The
  distribution's own **Range** is greyed (*Set by the linked plot*), and
  zooming and panning of the values are off, also before the plot shows a y
  range.

Plots of the same name are numbered in the menu, e.g. *Speed (1)* and
*Speed (2)*. Choose *None* to unlink: the distribution keeps the traces it
showed. Deleting the plot drops the link.

Without the link, an automatic distribution fits the samples in the shown time
range, while the plot fits all data of its traces, so a sideways distribution
lines up with the plot only while the range holds all values; link it to the
plot to line them up.

Heatmap
-------

.. figure:: /screenshots/view-heatmap-grid.png
   :target: ../_images/view-heatmap-grid.png
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
   :target: ../_images/view-logs.png
   :class: screenshot
   :alt: A log view with times, coloured levels, components and messages.

A table of the received log records with time, level, component and message,
of the paths checked in the tree (see :doc:`/blip/traces`); a new log view shows
all. While the shown records are of several paths, a **Path** column tells them
apart. Records outside the shown time range are greyed out. The filters are in the
column headers and are kept with the view:

* **Level** and **Component** -- checkboxes for what is shown, with **All**
  and **None**. New components are shown. Only the components of the shown
  paths are listed.
* **Message** -- a search, as a regular expression; **Aa** matches case. Text
  that is no valid regular expression is searched as it is.

Below the table, the arrow button follows the newest records, the count says
how many of the records of the shown paths are shown, and **Clear filters**
shows all again.

Properties: **Columns** -- **Date** shows the date with the time (of times in
Unix time, not of times since boot), and **Component** shows the component
column. While it is hidden, so is its filter, which then hides nothing.

Actions
-------

A form of the checked actions, to carry them out on their targets. See
:doc:`/blip/actions`.

Next steps
----------

* :doc:`/blip/layout` -- add, arrange and save views.
* :doc:`/blip/time` -- choose the time the views show.
