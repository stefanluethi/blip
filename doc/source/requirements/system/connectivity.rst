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

   blip shall show all samples on one time axis in one time base, UTC or the
   time since the target booted, that of the first sample as declared by the
   ``Hello`` of its connection or else guessed (:need:`REQ_PROTO_TIME`), and
   drop samples of the other base with a warning. When two connections declare
   different clocks, blip shall warn once, the UTC clock shall decide the
   shared axis, which shows the time of day, and the data of the time since
   boot shall go: when a connection with the UTC clock appears, blip shall
   remove the data of that base already received, and drop the later samples
   of it. The outpost shall keep the timestamps of a target going
   forward when the target restarts and its clock starts over, shifting those
   of the new run to follow the ones before, also those of ``Samples`` blocks,
   log the restart, and not time what it forwards with the time of its
   computer while the target sends its own.

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

.. req:: Log records
   :id: REQ_PROTO_LOG
   :status: implemented
   :component: proto
   :satisfies: STK_LIVE_OBSERVE
   :rationale: The path tells the sources of log records apart, so a record
      has no component. Old recordings and targets still send one.

   A ``Log`` record shall carry a timestamp, a path that is left out when it
   is empty, a level and a message, and no component. blip shall accept a
   record that still has a ``component`` field and ignore the field.

.. req:: Log API of sonde
   :id: REQ_SONDE_LOG
   :status: implemented
   :component: sonde
   :satisfies: STK_LIVE_OBSERVE, STK_DEVICE_IF

   The log functions of the sonde library shall take no component. A log
   record of a ``Tracer`` shall be at the name of the tracer.

.. req:: Typed trace values
   :id: REQ_PROTO_TYPES
   :status: implemented
   :component: proto
   :satisfies: STK_LIVE_OBSERVE

   The value of a ``Sample`` shall be an integer, float, bool, string, array
   or record, or ``null`` for no value, encoded as a plain CBOR data item
   without type tags.

.. req:: Hello of a connection
   :id: REQ_CONN_HELLO
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall keep the ``Hello`` a connection sends, and show its target name,
   its sender and its clock on the connection. It shall take the time base of
   the data of that connection from the clock of the ``Hello`` instead of
   guessing it. A second ``Hello`` on the same connection shall replace the
   first.

.. req:: Hello in the connections popup
   :id: REQ_CONN_HELLO_SHOW
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   The row of a connection in the connections popup shall show the target name
   right aligned on its first line, left out if it equals the name of the
   connection, and on its third line the sender and the clock as the time of
   day or the time since boot, such as ``sonde 1.2, time since boot``. After the
   connection is closed the lines of the ``Hello`` shall stay, faint, until
   the next ``Hello``.

.. req:: Newer protocol
   :id: REQ_CONN_NEWER
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: A target on a newer protocol may send what blip does not show;
      the user should know, but blip should show what it can.

   When the version of a ``Hello`` is newer than the protocol blip knows, blip
   shall warn once for each time the connection is made, in the log and as a
   notice, and keep reading the messages. While connected, the status dot of
   the connection, in its row and in the chip, shall be amber (warn) and its
   hover text and the third line of its row shall say ``protocol N is newer
   than blip`` (in warn colour), with the version N of the ``Hello``.
