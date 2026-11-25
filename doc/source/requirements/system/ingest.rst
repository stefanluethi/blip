Typed store entry
=================

What puts data into a store of ``blip-kit`` is typed: samples, blocks of
samples, log records, trace metadata, announced actions and the clock of a
source. An application may have data that never was CBOR, and it needs an input
other than CBOR. The blip protocol is one way into the store, read by the
library. The related requirements of sources and of the session are on
:doc:`library`, those of recordings on :doc:`files`.

Typed entry
-----------

.. req:: Typed entry of a store
   :id: REQ_LIB_INGEST_TYPED
   :status: implemented
   :component: blip-kit
   :priority: high
   :satisfies: STK_OWN_DATA
   :rationale: An application built on the library may have data that never was CBOR,
      and a store of its own must not need to read it.

   An application shall be able to put data into a store as typed data: the
   value of a path at a time, a block of values of a path at a period or at
   given times, a log record, the metadata of a trace, the actions a source
   announces and the clock of a source. The value of a sample shall be of any
   kind the protocol allows: an integer, float, bool, string, array, record, or
   none. A store, and what it keeps, shall take and need neither CBOR nor the
   bytes of a message.

.. req:: Bytes and typed data give the same
   :id: REQ_LIB_INGEST_SAME
   :status: implemented
   :component: blip-kit
   :priority: high
   :satisfies: STK_OWN_DATA, STK_LIVE_OBSERVE
   :rationale: The two ways into a store must not differ, or an application would
      see other data from a source that sends the protocol than from one that hands
      over the same data typed.

   The store, the invoker, the notices and the recording shall hold the same for
   the blip-protocol bytes of a message as for the same message handed over as
   typed data, for each kind of message: ``Hello`` (with each clock and with
   none), trace metadata, actions, the result of an action, a log record with and
   without a path, a sample of each kind of value, a block of samples with a
   period and with timestamps, and the old names ``Value`` and ``ValueMetadata``
   as the new ones. This shall hold while recording and while not recording.

.. req:: Latest values while not recording
   :id: REQ_LIB_INGEST_LATEST
   :status: implemented
   :component: blip-kit
   :priority: high
   :satisfies: STK_LIVE_OBSERVE, STK_OWN_DATA
   :rationale: Not recording must stay cheap, and the readbacks of actions must keep
      working, for the sources of an application as for a connection.

   While not recording, ``blip-kit`` shall not decode samples in full, keep no
   samples and no log records, and still keep the latest value of each path, so
   that the readbacks of an action show it as they do while recording, also for a
   value that arrived before its path became a readback. The author of a source
   shall need to do nothing for this: a source shall work the same, recording or
   not.

.. req:: Internals of the store are not API
   :id: REQ_LIB_INGEST_PRIVATE
   :status: implemented
   :component: blip-kit
   :priority: medium
   :satisfies: STK_OWN_DATA, STK_OWN_UI
   :rationale: The CBOR reader, the decoder and the tables are how the in-memory
      store works, not what an application builds on; public, they cannot change
      without breaking it.

   ``blip-kit`` shall not offer the internals of the store as public API: the CBOR
   reader, the decoder of samples, and the tables that grow and keep their
   timestamps sorted. The lookups a view of an application needs in a series or a
   trace of the store, such as the index at a time, the sample closest to a time
   and the samples within a range, shall be available on the series and trace
   types themselves.

.. req:: No change for the blip user by the typed entry
   :id: REQ_LIB_INGEST_NO_CHANGE
   :status: implemented
   :component: blip, blip-kit
   :priority: high
   :satisfies: STK_LIVE_OBSERVE
   :rationale: The typed entry is a restructuring; the user of blip shall not notice
      it.

   blip shall behave and look as before the typed entry: its traces, logs and
   actions, the readbacks of actions while not recording, the notices, and
   opening and saving recordings.

Connections as sources
----------------------

.. req:: Connections are sources of the session
   :id: REQ_LIB_CONN_SOURCES
   :status: approved
   :component: blip, blip-kit
   :priority: medium
   :satisfies: STK_OWN_UI, STK_OWN_DATA
   :rationale: Two paths into the store, one for the connections of blip and one for
      the sources of an application, would have to be kept alike; a connection of
      blip is what an application's WebSocket source is.

   blip shall hold its connections as sources of the session, so that the data of
   a connection reaches the store, the recording and the invoker the same way as
   that of any other source: there shall be one path from a connection into the
   store, and the connections shall not read or split the messages beside the
   session.

.. req:: Sources of the same name
   :id: REQ_LIB_SOURCE_SAME_NAME
   :status: approved
   :component: blip-kit
   :priority: medium
   :satisfies: STK_OWN_DATA, STK_LIVE_OBSERVE
   :rationale: Two connections that the user did not name, or two sources of an
      application, may have the same name.

   The session shall hold several sources of the same name, such as two
   connections with no name, and tell them apart: what each hands over, its
   status, its notices and the requests sent to it shall be its own, and the
   closing, the failure or the removal of one shall leave the others as they are.

.. req:: No change for the user by connections as sources
   :id: REQ_LIB_CONN_NO_CHANGE
   :status: approved
   :component: blip
   :priority: high
   :satisfies: STK_LIVE_OBSERVE
   :rationale: Moving the connections into the session is a restructuring; the user
      of blip shall not notice it.

   blip shall show the statuses of its connections, the ``Hello`` of a connection
   in the connections popup, the warning of a newer protocol and the notices of a
   connection that fails, breaks or is closed as before connections were sources
   of the session (:need:`REQ_CONN_HELLO_SHOW`, :need:`REQ_CONN_NEWER`,
   :need:`REQ_CONN_ERRORS`).
