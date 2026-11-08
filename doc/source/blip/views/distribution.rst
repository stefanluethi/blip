Distribution
============

.. figure:: /screenshots/view-distribution.png
   :target: ../../_images/view-distribution.png
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
  336*. The tooltip looks like those of the other views.
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
-----

.. figure:: /screenshots/view-distribution-boxes.png
   :target: ../../_images/view-distribution-boxes.png
   :class: screenshot
   :alt: A box plot of the robot's x and y position, a row per trace with its name on the left, each a box with a median line, a mean dot, whiskers and outlier dots.

With **Show** set to *Boxes* each trace has a row with its name on the
category axis; long names are cut with *…* at a third of the view. A box
spans the first to the third quartile as a light area under an outline, with a
line at the median, a dot at the mean, whiskers with caps, and a dot for each
outlier beyond the whiskers. Sideways, the traces are columns with their names
below.

.. figure:: /screenshots/view-distribution-both.png
   :target: ../../_images/view-distribution-both.png
   :class: screenshot
   :alt: The bars of the robot's x and y position with a box for each trace in the strip above them.

With *Both* the boxes are drawn in the strip above the bars (to the right of
them sideways), in rows of 14 pt, and replace the mean and σ markers.

.. figure:: /screenshots/view-distribution-boxes-sideways.png
   :target: ../../_images/view-distribution-boxes-sideways.png
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
   :target: ../../_images/panel-properties-distribution.png
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
--------------

.. figure:: /screenshots/view-distribution-sideways.png
   :target: ../../_images/view-distribution-sideways.png
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
