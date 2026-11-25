Views
=====

What the views show and how blip displays values.

.. req:: Plot value traces
   :id: REQ_VIS_PLOT
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall display received ``Sample`` and ``Samples`` messages as
   time-series plots, grouped by their ``path``.

.. req:: Show log records
   :id: REQ_VIS_LOG
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall display received ``Log`` messages in a table showing timestamp,
   path, level and message. The date of the timestamp can be shown or hidden.

.. req:: Log records without a path
   :id: REQ_VIS_LOG_DEFAULT
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: The path tells the sources of log records apart. A record
      without one still needs a place in the tree of the traces.

   blip shall show a log record that has no path at ``log/default``. blip
   shall keep any other path exactly as the target sends it, so ``log/imu``
   stays ``log/imu`` and ``motor`` stays ``motor``.

.. req:: Pick log paths by folder
   :id: REQ_VIS_LOG_PICK
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   When the user checks a folder of the tree of the traces, e.g. ``log``, a
   log table shall show the records of all log paths below it, also of paths
   that appear later.

.. req:: Filter the log table by path
   :id: REQ_VIS_LOG_PATH_FILTER
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: With the component gone, the path is how the user narrows a
      log table to some of its sources.

   The header of the path column of a log table shall offer a filter like the
   one of the level: a checkbox for each path of the shown records, written
   relative to the folder all of them share (``default``, ``imu`` and
   ``motor/left`` for ``log/default``, ``log/imu`` and ``log/motor/left``), and
   buttons to check all or none. The paths the user unchecked
   shall be saved with the layout, so that paths that appear later are shown.
   Clearing the filters of the table shall also clear the path filter. A layout
   saved with hidden components shall load without them and without a notice.

.. req:: Follow the newest log record
   :id: REQ_VIS_LOG_FOLLOW
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: A log that jumps to the newest record while the user reads an
      older one cannot be read.

   While live, a log table shall show the newest record and keep it in view as
   records arrive, until the user scrolls up from it. Scrolling back to the
   bottom or the button in the footer shall follow it again. While not live, the
   table shall move to the newest record of the shown time range as the range
   moves only if its autoscroll is on. The table shall show only whole rows.

.. req:: Log records and the time cursor
   :id: REQ_VIS_LOG_CURSOR
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   A click on a record of a log table shall put the time cursor at its time. The
   record at the time cursor shall be marked, and shall scroll into view when
   the cursor is put elsewhere, such as on a plot.

.. req:: Log message rows
   :id: REQ_VIS_LOG_ROWS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   A log table shall show each message on one line, cut at the end, with the
   whole text on hover, and offer Copy message and Copy row in the context menu
   of a record. A log table without checked logs shall say "Check the logs to
   show in the tree on the left", and one whose checked logs have no record in
   the shown time range "No entries in this time range".

.. req:: Show states over time
   :id: REQ_VIS_STATES
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall display string, bool and integer traces over time as a row of
   coloured bands, one band for each run of equal values. A name of a row too
   long for the axis shall end in "…".

.. req:: Show values in a table
   :id: REQ_VIS_TABLE
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall display the values of traces of any type in a table, at the
   time of the time cursor, also as a row per record with a column per field.
   Hovering a value shall show it with all its digits, and the context menu of
   a row shall offer to copy the value or the whole row, with its cells
   separated by tabs. The user shall be able to hide the traces without a value
   at the time of the cursor with the property Show empty traces.

.. req:: Plot traces against each other
   :id: REQ_VIS_XY
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall plot numeric traces against another numeric trace, pairing
   each sample with the sample of the other trace current at its time.

.. req:: Show arrays as heatmaps
   :id: REQ_VIS_HEATMAP
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall display array traces of one dimension as heatmaps over time
   and arrays of two dimensions as heatmaps at a point in time.

.. req:: Tooltips of the plot-like views
   :id: REQ_VIS_TOOLTIP
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: One look for what a hover shows lets the user read any view's
      tooltip the same way.

   The tooltips of Plot, States, Heatmap (bands, grid and the array line), XY
   and Distribution (bins and boxes) shall look alike: a framed popup at the
   pointer, never a frameless label. On top a header: the time, the bin range
   or the element index in weak text, or the dot and name of a trace. Below it
   rows with the dot and name of a trace, or a weak label, on the left and the
   value on the right, right aligned in a column as wide as its widest text,
   with its unit. A missing or empty value shall be faint. A rule may split
   groups of rows. XY and the array line of a Heatmap shall show the name of
   the trace with its x and y values, and the element index with its value.
   The time in a header shall have as many decimals as the zoom of the view
   needs to tell neighbouring pixels apart, and a Plot whose rows share the
   time of one sample shall show the time of that sample.

.. req:: Exact integers
   :id: REQ_VIS_INT
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall keep and show integer values exactly, over the whole range of
   the wire protocol from ``i64::MIN`` to ``u64::MAX``.

.. req:: Zoom the UI
   :id: REQ_VIS_ZOOM
   :status: implemented
   :component: blip

   blip shall let the user scale its whole UI, with Zoom in, Zoom out and
   Reset zoom, which names the current scale in per cent, in the View menu, and
   keep the scale across restarts.

.. req:: Views without anything to show
   :id: REQ_VIS_EMPTY
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: One wording tells the user what to do in every view.

   A view with no trace checked shall say "Check the traces to show in the tree
   on the left", an XY view with a single trace "Check a second trace to plot
   against the first", and a log table the hints of :need:`REQ_VIS_LOG_ROWS`.
