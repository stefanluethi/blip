XY
==

.. figure:: /screenshots/view-xy.png
   :target: ../../_images/view-xy.png
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
