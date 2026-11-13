Trace metadata
==============

What blip shows of the metadata a target declares for its traces: the
description, the labels of integer values and the expected range
(:need:`REQ_PROTO_METADATA`).

.. req:: Labels of integer values
   :id: REQ_META_LABELS
   :status: implemented
   :component: blip
   :satisfies: STK_MEANING, STK_LIVE_OBSERVE
   :rationale: A state machine sends 3, the user wants to read ``Running``.

   blip shall offer the number format Labels for a trace, the first of the
   formats and the default for a trace that has labels. With it the label of an
   integer shall replace the integer in the bands of States, the cells of a
   table, the tooltips and the value of the legend at the cursor. The colour
   of a band shall follow the integer, not the label, except that a label that
   is the name of a log level takes the colour of that level, as such strings
   do today. An integer without a label shall be shown as the plain number, a
   float with a whole value shall get the label of that integer, and a bool
   shall ignore the labels. The other number formats shall stay as they are.

.. req:: Facts of the traces in the properties
   :id: REQ_META_PROPS
   :status: implemented
   :component: blip
   :satisfies: STK_MEANING, STK_LIVE_OBSERVE

   The properties of a view that writes values shall have a section Traces in
   place of Number format, with a block for each shown trace: the choice of
   its number format, its unit, its description wrapped, ``Expected <min> to
   <max>`` for its expected range, and its labels in one line, cut at the
   end, with all of them listed in the hover text. A fact that is missing shall leave no
   line. In the Records layout of a table the same lines shall be under each
   column in Columns.

.. req:: Labels and descriptions in tooltips
   :id: REQ_META_TOOLTIP
   :status: implemented
   :component: blip
   :satisfies: STK_MEANING, STK_LIVE_OBSERVE
   :rationale: Builds on :need:`REQ_VIS_TOOLTIP`; the look of the tooltip stays
      the same.

   In the tooltips of the plot-like views a value that has a label shall have
   the raw number next to it in a faint column. Below a rule a footer shall
   show the name and description of the one trace under the pointer, in at most
   3 lines, and none if it has no description. That trace is, in States, the
   band row; in Plot, the nearest curve within about 12 px; in Heatmap, XY and
   Distribution, the trace of the header.

.. req:: Description on hover in the trees and tables
   :id: REQ_META_HOVER
   :status: implemented
   :component: blip
   :satisfies: STK_MEANING, STK_LIVE_OBSERVE

   Hovering a trace in the tree of the traces, the name of a trace in a table
   and the header of a column in the Records layout of a table shall show its
   path, its type and its description.

.. req:: Fit the plot axis to the expected range
   :id: REQ_META_FIT_PLOT
   :status: implemented
   :component: blip
   :satisfies: STK_MEANING, STK_LIVE_OBSERVE
   :rationale: A trace whose range is known, such as 0 to 100 %, should not
      make the axis jump with its values.

   While a shown trace of a plot declares an expected range, the Y axis
   properties shall have a row Fit to with the choices Expected and Values,
   Expected by default; without such a trace the row shall not be shown and
   the axis fit the values as before. Expected shall fit the union of the
   declared ranges and the fitted values of the shown traces that declare
   none; values beyond the expected range shall grow the axis. Values shall fit
   the values of the shown traces. The fields Min and Max shall show the ends
   in use as faint hints; typing in a field shall override that end and
   clearing it return to the fit. A Distribution linked to the plot shall take
   the range of its axis as it does today.

.. req:: Fit the XY axes to the expected ranges
   :id: REQ_META_FIT_XY
   :status: implemented
   :component: blip
   :satisfies: STK_MEANING, STK_LIVE_OBSERVE

   While a shown trace of an XY view declares an expected range, its Axes
   properties shall have the same Fit to row, with the X axis taken from the
   range of the X trace and the Y axis from the union of the ranges of the Y
   traces, as in :need:`REQ_META_FIT_PLOT`.

.. req:: Heatmap colour scale from the expected range
   :id: REQ_META_HEATMAP
   :status: implemented
   :component: blip
   :satisfies: STK_MEANING, STK_LIVE_OBSERVE

   For an array trace that declares an expected range, the colour scale Auto of
   a Heatmap shall be that range, and Fixed shall start from it, its min and
   max showing the range when the user switches to Fixed.

.. req:: Metadata settings saved
   :id: REQ_META_SAVED
   :status: implemented
   :component: blip
   :satisfies: STK_MEANING, STK_REVIEW

   blip shall save the number format of each trace, Labels among them, and the
   Fit to choices with the layout of the preset, and restore them when the
   layout is loaded. A layout saved before shall load without a notice.
