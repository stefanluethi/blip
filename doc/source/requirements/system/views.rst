Views
=====

What the views show and how blip displays values.

.. req:: Plot value traces
   :id: REQ_VIS_PLOT
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall display received ``Value`` messages as time-series plots,
   grouped by their ``path``.

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

.. req:: Show states over time
   :id: REQ_VIS_STATES
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall display string, bool and integer traces over time as a row of
   coloured bands, one band for each run of equal values.

.. req:: Show values in a table
   :id: REQ_VIS_TABLE
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall display the values of traces of any type in a table, at the
   time of the time cursor, also as a row per record with a column per field.

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

   blip shall let the user scale its whole UI, and keep the scale across
   restarts.
