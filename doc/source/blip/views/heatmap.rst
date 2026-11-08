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
  thermal camera. Cells wide enough show their value.

Properties: **Array** -- **Show** *Over time* or *At cursor* (1D arrays), and
**Colour scale** -- fitted to the shown values, or fixed between a min and a
max.
