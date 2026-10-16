System Requirements
===================

Visualisation
-------------

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
   level, component and message.

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
   time of the time cursor.

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

.. req:: Time cursor
   :id: REQ_VIS_CURSOR
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall show one time cursor in all views over time, which the user can
   set and drag, and which stays at the end of the shown time range until set.

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

Connectivity
------------

.. req:: WebSocket connection
   :id: REQ_CONN_WS
   :status: implemented
   :component: blip, bridge
   :satisfies: STK_LIVE_OBSERVE

   blip shall connect to user-configurable WebSocket addresses, several at
   once, and receive messages as binary frames, keeping the data of each
   connection apart by its name.

.. req:: CBOR wire protocol
   :id: REQ_PROTO_CBOR
   :status: implemented
   :component: proto
   :satisfies: STK_LIVE_OBSERVE, STK_CONTROL

   Messages shall be encoded as CBOR according to the types defined in the
   ``proto`` crate.

.. req:: Typed trace values
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

   blip shall persist the layout of the views and the target addresses across
   restarts.

Files
-----

.. req:: Save and open recordings
   :id: REQ_FILE_RECORDING
   :status: implemented
   :component: blip
   :satisfies: STK_REVIEW

   blip shall save the messages received since the data was last cleared to
   a recording file, and open recording files, showing their data as if it
   had been received.

.. req:: Save and open layouts
   :id: REQ_FILE_LAYOUT
   :status: implemented
   :component: blip
   :satisfies: STK_REVIEW

   blip shall save the layout of the views to a JSON file and open layout
   files saved by the same or an older version of blip.

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

.. req:: Record messages
   :id: REQ_BRIDGE_RECORD
   :status: implemented
   :component: bridge
   :satisfies: STK_REVIEW

   The bridge shall record the messages it sends to blip to a recording file
   that blip opens.

.. req:: Replay recordings
   :id: REQ_BRIDGE_REPLAY
   :status: implemented
   :component: bridge
   :satisfies: STK_REVIEW, STK_DEVICE_IF

   The bridge shall send the messages of a recording file to blip unchanged,
   at the pace of their timestamps or faster.
