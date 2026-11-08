Histogram
=========

The histogram view.

.. req:: Show the distribution of values
   :id: REQ_VIS_HIST
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall offer a Distribution view, shown as "Distribution" in the view
   picker, the menus and as default title; presets saved with histogram
   views shall load as Distribution views unchanged. In its Bars form, the
   histogram, the view counts the samples of each
   checked trace in bins of their values over the shown time range, and
   counts anew as the range is panned, zoomed or moves on while live. The
   user shall be able to check integer and float traces and the elements of
   arrays of up to 16 elements, but not bool and string traces. Null samples
   shall not be counted. The traces shall be drawn over each other on one
   shared set of bins, each in its colour, as a translucent area under an
   outline in the full colour. The legend shall show a dot and the name of
   each trace, without a value, in the title row of the view, or in its top
   right corner without a title row. The value axis, the bin width, the bin
   edges in the tooltip and the legend in the title row shall show the unit
   of the traces only if all checked traces share one. The bin edges and the values on the
   axis shall be written in the number format of the traces if all have the
   same one, else in the default format.

.. req:: Histogram bins
   :id: REQ_VIS_HIST_BINS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall let the user choose the bins of the Bars of a Distribution
   view, the histogram: automatic, a
   fixed number of bins, or a fixed bin width in the unit of the traces.
   Automatic bins shall be about the square root of the number of samples,
   at least 8 and at most 64, with a width rounded to 1, 2 or 5 times a power
   of ten, over the values shown with a margin of 5 % on each side, and the
   width shall change only once the ideal width is clearly past a step, so
   it does not flip between two widths while live. The edges of automatic
   bins and of bins of a fixed width shall lie on multiples of the bin
   width, so the bars do not shift as the shown time range moves; a fixed
   number of bins shall split the range of the values evenly. Bins of a
   whole-number width over integer traces shall have their edges half a unit
   below the multiples of the width instead, so no integer lies on an edge
   and each bin holds the same number of integers; with a width of 1, the
   bar of a value runs from half a unit below it to half a unit above it.
   The user
   shall be able to fix the minimum and maximum of the values binned;
   samples outside a fixed range shall not be binned but counted, and their
   counts shown faintly at the end they lie beyond, such as "‹ 232" and
   "36 ›". Infinite samples shall not be binned but counted the same way at
   their end, also when the range is not fixed.

.. req:: Histogram counts axis
   :id: REQ_VIS_HIST_COUNTS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   The counts axis of the Bars of a Distribution view, the histogram, shall show the samples of each trace in a
   bin as a percentage of all samples of that trace in the shown time range,
   those outside the bins included, so traces of different sample rates can
   be compared. It shall fit the tallest bin in view, and the user shall be
   able to switch it between a linear and a logarithmic scale. The
   logarithmic scale shall have a tick at each power of ten and start at
   least half a decade below the percentage of one sample of the trace with
   the most samples, rounded down to half a decade, so a bin of one sample
   shows as a short bar and an empty bin shows nothing.

.. req:: Histogram mean and spread
   :id: REQ_VIS_HIST_MARKERS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall let the user show, for all traces of the Bars of a Distribution
   view at once,
   no markers (the default), the mean, or the mean and the population
   standard deviation σ (divided by the number of samples, as in the plot
   statistics) of the samples binned, counting each sample once whatever
   time it lasts. They shall be shown in a strip at the far end of the
   counts axis, clear of the bars, with a row per trace: a dot at the mean
   and a whisker from the mean −σ to the mean +σ, in the colour of the
   trace.

.. req:: Distribution orientation
   :id: REQ_VIS_HIST_ORIENT
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall let the user show a Distribution view, in all its forms,
   upright, with the values along the horizontal axis and the bars growing
   up or the boxes stacked in rows, or sideways, with the values along the
   vertical axis and the bars growing to the side or the boxes side by side
   in columns. The value axis of a sideways Distribution view shall be on the left, as in a plot, or on the
   right, as the user chooses.

.. req:: Distribution linked to a plot
   :id: REQ_VIS_HIST_LINK
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: A Distribution view beside a plot shows the distribution of the
      plot's traces over the plot's y range; sideways, at the same heights as
      the plot. Taking the plot's traces saves checking them twice.

   blip shall let the user link a Distribution view, upright or sideways and in
   all its forms, to a plot
   view of the layout, or to none. While linked, the view shall show
   the traces the plot shows, with the plot's number formats, as they
   change, and the tree of the traces shall show them checked but greyed,
   with a hint naming the plot; its number formats shall be greyed too.
   Unlinked, the view shall keep the traces it showed last. While
   linked, the value axis of the view shall show the y range the plot
   shows, as it is zoomed, panned or fixed, and that range shall act as a
   fixed range: the boxes and whiskers shall be cut at its edges and the bins, automatic, a fixed number or a fixed width, shall
   cover it, and samples outside it shall be counted and shown faintly at the
   end they lie beyond. The view's own fixed range, zooming and panning
   of the values shall be off while linked, also while the plot shows no y
   range yet. Deleting the plot shall drop the link.

.. req:: Distribution interaction
   :id: REQ_VIS_HIST_HOVER
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   Hovering over a bin of the Bars of a Distribution view shall highlight the bin and show its
   range of values, with the unit the traces share, and for each trace its
   percentage of the samples of the trace and its count of them, such as
   "12.5 %" and "42 of 336". The
   mouse wheel shall zoom, and dragging shall pan, the value axis only, of
   all forms, magnifying the bins without binning anew, and a double click shall fit it
   again. None of them shall change the shown time range or the time cursor.

.. req:: Distribution without data
   :id: REQ_VIS_HIST_EMPTY
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   A Distribution view without checked traces shall show a hint to check
   traces in the tree of the traces. A Distribution view whose traces have no samples in the
   shown time range shall show empty axes and "No samples in the shown
   window". The Bars of a view whose samples all have the same value shall
   show one bar centred on that value.

.. req:: Distribution settings saved
   :id: REQ_VIS_HIST_SAVED
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE, STK_REVIEW

   blip shall save the checked traces and all settings of a Distribution
   view, its show form (Bars, Boxes or Both), whiskers, bins, range, counts
   scale, markers, orientation, value axis side and the
   plot it is linked to, with the view in the layout of the preset, and restore
   them when the layout is loaded.
