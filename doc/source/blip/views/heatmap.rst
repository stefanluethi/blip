Heatmap
=======

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
  thermal camera. Cells wide enough show their value. The grid fills its card;
  its cells are square while that leaves them more than half of it, and else
  stretched, at most three times as long as wide.

.. figure:: /screenshots/view-heatmap-grid-tooltip.png
   :target: ../../_images/view-heatmap-grid-tooltip.png
   :class: screenshot
   :alt: The tooltip of a heatmap grid, with the cell's row and column and its value.

Hover a cell or band for a tooltip: over time it shows the time and the value
of element ``[i]``; a grid shows ``[r][c]`` and the value of the cell; the
line at the cursor shows ``[i]`` and the value, with the trace's dot and name.
Below a rule it gives the trace's description, if the target sent one.

Hover values show 4 digits. The heatmap takes one trace, so the tree has radio
buttons for it.

Properties: **Array** -- **Show** *Over time* or *At cursor* (1D arrays), and
**Colour scale** -- **Fit to** (while the trace declares a range) and **Min**
and **Max**, empty to fit, as in the other views. With **Fit to** *Expected*,
the scale grows to take in values beyond the expected range.

A 1D array shown at the cursor has a section **Y axis** in place of the colour
scale, with the same rows. Its y zoom stays until you double-click, and it has
**Lines** with **Points** and **Width**, and **Line styles**.
