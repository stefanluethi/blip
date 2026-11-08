Connectivity
============

Connections to targets and the wire protocol.

.. req:: WebSocket connection
   :id: REQ_CONN_WS
   :status: implemented
   :component: blip, outpost
   :satisfies: STK_LIVE_OBSERVE

   blip shall connect to user-configurable WebSocket addresses, several at
   once, and receive messages as binary frames. It shall show the paths of
   traces, logs and actions as the targets send them, leaving their
   organization to the user, keep the samples of a path several targets send
   in the order of their timestamps, and send the actions to the connection
   chosen for them, by default each to the first with an open connection
   announcing it. While recording, blip shall keep
   the user from connecting, disconnecting and editing the connections, but
   for connecting again those that broke. A connection opening while no other is
   open shall start afresh, without the data, traces and actions received or
   opened before; blip shall ask before while a recording is not saved.

.. req:: One timeline
   :id: REQ_CONN_TIMELINE
   :status: implemented
   :component: blip, outpost
   :satisfies: STK_LIVE_OBSERVE

   blip shall show all samples on one time axis in the time base of the first,
   UTC or the time since the target booted, and drop samples of the other base
   with a warning. The outpost shall keep the timestamps of a target going
   forward when the target restarts and its clock starts over, shifting those
   of the new run to follow the ones before, log the restart, and not time
   what it forwards with the time of its computer while the target sends its
   own.

.. req:: Notify errors
   :id: REQ_CONN_ERRORS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall tell the user when connecting fails, a connection breaks or is
   closed by the other end, and when a file cannot be opened or saved, with a
   notice that disappears on its own.

.. req:: CBOR wire protocol
   :id: REQ_PROTO_CBOR
   :status: implemented
   :component: proto
   :satisfies: STK_LIVE_OBSERVE, STK_CONTROL

   Messages shall be encoded as CBOR according to the types defined in the
   ``proto`` crate.

.. req:: Several messages per frame
   :id: REQ_PROTO_SEQUENCE
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: Targets send their messages in batches, e.g. one UDP datagram
      per control cycle.

   blip shall accept a binary frame holding several messages as a CBOR
   sequence (RFC 8742), and apply them in their order. A frame that cannot be
   read to its end shall keep the messages before.

.. req:: Typed trace values
   :id: REQ_PROTO_TYPES
   :status: implemented
   :component: proto
   :satisfies: STK_LIVE_OBSERVE

   A ``Value`` shall carry an integer, float, bool, string, array or record,
   or ``null`` for no value, encoded as a plain CBOR data item without type
   tags.
