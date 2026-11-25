XY and Heatmap
==============

The properties of the XY plot and of the Heatmap of an array at the cursor.

.. req:: XY axes, curves and legend
   :id: REQ_VIS_XY_PROPS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   The properties of an XY view shall have the sections X axis and Y axis, each
   with the range of :need:`REQ_VIS_RANGE` and a Label, which names the traces
   and the unit on the axis while empty; Lines with their Width; and Legend with
   its Placement, in the title row or in a corner. The section Axes shall have
   a row for each curve with the choice of its X and its Y trace; choosing a
   trace already used on the other side shall swap the two. XY shall show no
   hover rulers. XY plots saved before shall load to the same picture.

.. req:: Heatmap of an array at the cursor
   :id: REQ_VIS_HEATMAP_PROPS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   A Heatmap showing arrays at the cursor as a line shall have the section Y
   axis with the range of :need:`REQ_VIS_RANGE`, and the properties Points
   (when the elements are marked), Width and the line styles of the traces.
   Its y zoom shall stay until a double click. A Heatmap showing a
   two-dimensional array as a grid shall fill its card, with square cells only
   while they fill more than half of it.
