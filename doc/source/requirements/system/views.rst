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
   level, component and message. The date of the timestamp and the component
   can be shown or hidden.

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
