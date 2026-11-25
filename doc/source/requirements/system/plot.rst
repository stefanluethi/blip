Plot and ranges
===============

How the plot-like views are read and steered: the axis ranges, double clicks
and drags, and the legends.

.. req:: Range of an axis
   :id: REQ_VIS_RANGE
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: Plot, XY, Heatmap and Distribution had each its own way to fix a
      range; one control is learnt once.

   Where the user sets the range of an axis, of the colour scale of a Heatmap
   or of the values a Distribution bins, the properties shall offer the same
   controls: a row Fit to with the choices Expected and Values, shown only
   while a shown trace declares an expected range (:need:`REQ_META_FIT_PLOT`),
   then the fields Min and Max. Each field shall be empty to fit, showing the
   end that fits as a faint hint, and shall set that end alone. Text that is no
   finite number shall stay in its field, outlined in the error colour with the
   accepted form on hover, and leave the end as it was. Ends set the wrong way
   round shall be outlined in the same way, with the reason on hover, and the
   view shall fit the values instead. Presets saved with a fixed range, or with
   none, shall load to the same picture.

.. req:: Zoom and fit the axes of a plot
   :id: REQ_VIS_PLOT_AXES
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   In the views over time a double click shall fit the y axis and leave the
   shown time range and the time cursor as they were. A drag shall pan
   only after it has moved a few points and then only in the direction it has
   gone, so that a vertical drag pans no time. While the y axis of a Plot is
   zoomed, its Min and Max fields shall show the range in use as their hints
   and a row Zoomed shall offer a button to fit the axis. The y axis of a Heatmap showing an array at the
   cursor shall stay where the user zoomed it until a double click.

.. req:: Legends of the plot-like views
   :id: REQ_VIS_LEGEND
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: A click on a legend that hides a trace hides what the user is
      looking for without a sign.

   A legend in a corner of a view shall only show the traces and not be
   clickable, so it cannot hide a trace. The legend in the title row of a view
   shall keep whole entries while they fit and show the unit the traces share
   once at its end. When the room is short it shall leave the unit out if the
   whole entries fit without it, else cut the name of the last entry that fits
   and leave out the entries after it for a "…". The value in the legend and
   the tooltip of a Plot shall be those of the same sample. Hovering a value of
   the legend or a statistic below the plot shall show it with all its digits.
