System Requirements
===================

Visualisation
-------------

.. req:: Plot value streams
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
   level, component and message.

.. req:: Show states over time
   :id: REQ_VIS_STATES
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall display string, bool and integer streams over time as a row of
   coloured bands, one band for each run of equal values.

.. req:: Show values in a table
   :id: REQ_VIS_TABLE
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall display the values of streams of any type in a table, at the
   time of the time cursor.

.. req:: Plot streams against each other
   :id: REQ_VIS_XY
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall plot numeric streams against another numeric stream, pairing
   each sample with the sample of the other stream current at its time.

.. req:: Show arrays as heatmaps
   :id: REQ_VIS_HEATMAP
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall display array streams of one dimension as heatmaps over time
   and arrays of two dimensions as heatmaps at a point in time.

.. req:: Time cursor
   :id: REQ_VIS_CURSOR
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall show one time cursor in all views over time, which the user can
   set and drag, and which stays at the end of the shown time range until set.

Connectivity
------------

.. req:: WebSocket connection
   :id: REQ_CONN_WS
   :status: implemented
   :component: blip, bridge
   :satisfies: STK_LIVE_OBSERVE

   blip shall connect to a user-configurable WebSocket address and receive
   messages as binary frames.

.. req:: CBOR wire protocol
   :id: REQ_PROTO_CBOR
   :status: implemented
   :component: proto
   :satisfies: STK_LIVE_OBSERVE, STK_CONTROL

   Messages shall be encoded as CBOR according to the types defined in the
   ``proto`` crate.

.. req:: Typed stream values
   :id: REQ_PROTO_TYPES
   :status: implemented
   :component: proto
   :satisfies: STK_LIVE_OBSERVE

   A ``Value`` shall carry an integer, float, bool, string, array or record,
   or ``null`` for no value, encoded as a plain CBOR data item without type
   tags.

.. req:: Send actions to the target
   :id: REQ_CTRL_ACTION
   :status: draft
   :component: blip, bridge
   :satisfies: STK_CONTROL

   blip shall send ``Action`` messages to the connected target, and the
   bridge shall forward them to the device.

Platforms
---------

.. req:: Web application
   :id: REQ_PLAT_WEB
   :status: implemented
   :component: blip
   :satisfies: STK_ZERO_INSTALL

   blip shall run in current desktop web browsers as a WebAssembly
   application.

.. req:: Native application
   :id: REQ_PLAT_NATIVE
   :status: implemented
   :component: blip

   blip shall be buildable as a native desktop application on Linux.

.. req:: Persist UI state
   :id: REQ_PERSIST_STATE
   :status: implemented
   :component: blip

   blip shall persist the layout of the views and the target address across
   restarts.

Bridge
------

.. req:: Serial bridge
   :id: REQ_BRIDGE_SERIAL
   :status: approved
   :component: bridge
   :satisfies: STK_DEVICE_IF

   The bridge shall forward COBS-framed CBOR messages between a serial port
   and WebSocket clients.

.. req:: Socket bridge
   :id: REQ_BRIDGE_SOCKET
   :status: implemented
   :component: bridge
   :satisfies: STK_DEVICE_IF

   The bridge shall forward messages between a TCP or UDP endpoint and
   WebSocket clients.

.. req:: Simulator
   :id: REQ_BRIDGE_SIM
   :status: verified
   :component: bridge
   :satisfies: STK_DEVICE_IF

   The bridge shall provide a simulator mode that streams synthetic CPU load
   and sensor values, and log records of all severity levels, without
   hardware.
