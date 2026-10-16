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

   blip shall connect to a user-configurable WebSocket address and receive
   messages as binary frames.

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

Control
-------

.. req:: Announce actions
   :id: REQ_CTRL_ANNOUNCE
   :status: implemented
   :component: proto, bridge
   :satisfies: STK_CONTROL

   The target shall announce the actions it can carry out with their
   parameters (bool, integer, float, choice or string, with bounds, unit and
   default), an optional readback trace, whether they need confirmation and
   whether they are enabled. Each announcement shall replace the previous one,
   and blip clients connecting later shall get the latest.

.. req:: Invoke actions
   :id: REQ_CTRL_ACTION
   :status: implemented
   :component: blip, bridge
   :satisfies: STK_CONTROL

   blip shall send invocations of announced actions with their arguments to
   the connected target. The bridge shall check the arguments against the
   parameters and carry out the invocations of an action one after another in
   the order they arrived.

.. req:: Action feedback
   :id: REQ_CTRL_FEEDBACK
   :status: implemented
   :component: blip, bridge
   :satisfies: STK_CONTROL

   The target shall answer each invocation to the client that sent it with
   whether it carried it out or the reason it rejected it. blip shall show
   whether the latest invocation of an action is pending, accepted, rejected
   with the reason, or unanswered after a timeout. The bridge shall log each
   invocation and its outcome to all clients.

.. req:: Actions view
   :id: REQ_CTRL_VIEW
   :status: implemented
   :component: blip
   :satisfies: STK_CONTROL

   blip shall offer a view of actions placed with the layout, showing buttons,
   checkboxes, choices, sliders, number and text fields by the kinds of their
   parameters, and the value of their readback traces. Sliders shall send
   while dragged at a limited rate, actions with several parameters shall be
   sent with a separate Send button.

.. req:: Actions only on request
   :id: REQ_CTRL_SAFE
   :status: implemented
   :component: blip
   :satisfies: STK_CONTROL

   blip shall send an action only when the user operates its input, never on
   its own at startup or on reconnecting, ask before sending actions that
   need confirmation, and disable actions while not connected or while the
   target cannot carry them out.

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

.. req:: User-provided protocols
   :id: REQ_BRIDGE_PROTOCOL
   :status: implemented
   :component: bridge
   :satisfies: STK_DEVICE_IF, STK_CONTROL

   The bridge shall load the protocol that talks to the target from a Python
   file or module provided by the user, configured with options converted by
   the type hints of its constructor. A protocol shall declare its actions as
   Python functions whose parameters are taken from their type hints, and may
   change them at runtime.

.. req:: Socket bridge
   :id: REQ_BRIDGE_SOCKET
   :status: implemented
   :component: bridge
   :satisfies: STK_DEVICE_IF

   The bridge shall forward messages between a TCP or UDP endpoint and
   WebSocket clients, and forward invocations to the endpoint.

.. req:: Simulator
   :id: REQ_BRIDGE_SIM
   :status: implemented
   :component: bridge
   :satisfies: STK_DEVICE_IF, STK_CONTROL

   The bridge shall provide a simulator mode that streams synthetic CPU load
   and sensor values, and log records of all severity levels, without
   hardware, and offers actions that change what it streams.

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
   except that recorded actions are disabled, at the pace of their timestamps
   or faster.
