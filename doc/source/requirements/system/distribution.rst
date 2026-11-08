Distribution
============

The distribution view and its boxes.

.. req:: Distribution shows bars, boxes or both
   :id: REQ_VIS_DIST_SHOW
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall let the user choose what a Distribution view shows: Bars (the
   default), Boxes or Both. Bars shall be the histogram with the Markers
   Off, Mean or Mean ±σ. Boxes shall be a box plot, in which each trace gets
   an equal row along the shared value axis, upright with the values along
   the horizontal axis and the rows stacked, sideways with the values along
   the vertical axis and the columns side by side. The names of the traces
   shall label the rows on the category axis, below the columns when
   sideways, cut with "…" at no more than a third of the view, with no
   lines or bands between the rows; the bins and counts settings shall not
   apply, but shall be kept, so switching back restores them. Both shall be the bars with a box per trace in the marker strip
   above them, in place of the mean and σ markers, with the rows of the strip
   grown from 10 pt to 14 pt. Orientation, axis side, link to a plot, value
   zoom, pan and fit by double click, units, number formats and the legend in
   the title row shall apply to all forms.

.. req:: Distribution box
   :id: REQ_VIS_DIST_BOX
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall draw the box of a trace from the first to the third quartile,
   with a median line and the mean as a dot. The box shall be a wash under
   an outline of 1.5 pt in the colour of the trace, the median line heavier
   than the outline, the mean dot ringed in the card colour, the whiskers
   capped, and the outliers small dots at 45 % opacity. The box shall be 40 %
   of its row thick, at most 24 pt, and in Both 10 pt in a 14 pt row, with
   an outline of 1 pt.

.. req:: Distribution whiskers
   :id: REQ_VIS_DIST_WHISKERS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall let the user choose, for Boxes and Both, where the whiskers end:
   1.5×IQR (the default), the furthest sample within 1.5 times the
   interquartile range of the box; Min–max; 5–95 %; or 1–99 %. Samples beyond
   the whiskers shall be drawn as outlier dots, none with Min–max.

.. req:: Distribution box statistics
   :id: REQ_VIS_DIST_STATS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall compute the box of a trace over all its non-null samples in the
   shown time range, each counted once and not weighted by the time it lasts,
   as the bars do. Quartiles and percentiles shall be linear interpolations
   between the order statistics (type 7, the default of numpy). They shall not
   depend on the range of the value axis: parts beyond a fixed or linked
   range shall be cut off at its edge, and a whisker end without cap shall
   read as cut. They shall be computed anew as the range is panned, zoomed or
   moves on while live.

.. req:: Distribution box interaction
   :id: REQ_VIS_DIST_BOX_HOVER
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   Hovering anywhere in the row of a trace in Boxes or in the box strip of Both
   shall light the row, as a bin column is lit, and show a tooltip with the dot
   and name of the trace and a grid of names and right-aligned values in the
   number format of the trace with its unit: min, low whisker, Q1, median,
   mean, Q3, high whisker and max, in the order drawn (low to high upright,
   high to low sideways), then under a rule the number of Samples and of
   Outliers. With Min–max, the rows of whiskers and outliers shall be left
   out; percentile whiskers shall be named by their percentile, such as "5 %"
   and "95 %".

.. req:: Distribution box without data
   :id: REQ_VIS_DIST_BOX_EMPTY
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   A trace without samples in the shown time range shall keep its row in Boxes
   and Both with its name and "No samples". A trace whose samples all have the
   same value shall have its box collapsed to the median line with the mean
   dot. A trace whose whole box lies beyond a fixed or linked value range
   shall show a faint arrow at the edge it lies beyond, as the bars show the
   samples outside the range faintly, and its tooltip shall still work.

.. req:: Distribution properties
   :id: REQ_VIS_DIST_PROPS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall group the properties of a Distribution view as Link (Plot);
   Display (Show, Orientation, Axis side when sideways, then Markers for Bars
   or Whiskers for Boxes and Both in the same place); Values (Range, which
   cuts the boxes too); and for Bars and Both only Bins (Mode, Number or
   Width) and Counts (Scale). Rows that do not apply to the form shown shall be
   hidden.
