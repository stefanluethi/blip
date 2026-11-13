Protocol
========

The contract of the wire protocol and its messages beyond the basics of
:need:`REQ_PROTO_CBOR`. The messages are described in
:doc:`/reference/protocol`.

.. req:: Timestamps and time base
   :id: REQ_PROTO_TIME
   :status: implemented
   :component: proto, blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: The time axis needs to know whether timestamps are a date or an
      uptime. A target says so in its Hello; older ones do not, and blip keeps
      guessing for them.

   The timestamps of ``Sample``, ``Samples`` and ``Log`` messages shall be in
   nanoseconds. The time base of the timestamps of a connection shall be the
   ``clock`` of its ``Hello``: ``"epoch"`` for the Unix epoch (UTC) or
   ``"boot"`` for the time since the target booted. For a connection that sent
   no ``Hello``, blip shall guess the time base from the timestamps as it does
   today: before 2001-09-09 (10^18 ns) the time since boot, else the Unix
   epoch.

.. req:: Log records without a time
   :id: REQ_PROTO_LOG_NOW
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall place a ``Log`` record with the timestamp 0 at the newest time of
   the data received so far, or, if there is none yet, at the time of day of
   the computer.

.. req:: Log levels
   :id: REQ_PROTO_LEVELS
   :status: implemented
   :component: proto, blip
   :satisfies: STK_LIVE_OBSERVE

   The level of a ``Log`` record shall be 0 Trace, 1 Debug, 2 Info, 3 Warn or
   4 Error. blip shall show a level above 4 as Error.

.. req:: Sample and TraceMetadata messages
   :id: REQ_PROTO_NAMES
   :status: implemented
   :component: proto, blip, outpost, sonde
   :satisfies: STK_LIVE_OBSERVE, STK_DEVICE_IF
   :rationale: The messages are about traces and samples, not about values in
      general. The old names stay readable so that old targets and recordings
      keep working.

   The proto crate, the outpost and sonde shall send the sample of a trace as
   a ``Sample`` message and the metadata of a trace as a ``TraceMetadata``
   message. blip shall also read the messages named ``Value`` and
   ``ValueMetadata``, which older senders and recordings have, as ``Sample``
   and ``TraceMetadata``, and the outpost shall treat them as such too. ``Log``
   and ``Data`` keep their names.

.. req:: Trace metadata
   :id: REQ_PROTO_METADATA
   :status: implemented
   :component: proto, blip
   :satisfies: STK_MEANING, STK_LIVE_OBSERVE

   A ``TraceMetadata`` message shall carry the path of a trace, and may carry
   its unit (an empty or missing unit is no unit), a description (text),
   labels for integer values and the expected range of the trace. The labels
   (``enum``) shall be a CBOR map from integers to text, the range (``range``)
   a map ``{min, max}`` of numbers. Metadata without the optional fields, as
   older senders send it, and with a unit only, shall keep working. Metadata shall apply to the trace at its path, also when
   that is the field trace of a record such as ``imu.acc``, and to the
   element traces of an array trace, such as ``imu.acc[0]``.

.. req:: Hello message
   :id: REQ_PROTO_HELLO
   :status: implemented
   :component: proto
   :satisfies: STK_LIVE_OBSERVE

   A target may send a ``Hello`` message with the version of the protocol it
   speaks, which is 1 for this revision, its name, the clock of its timestamps (``"epoch"`` or ``"boot"``)
   and its sender, the software and version that sends, such as
   ``"sonde 1.2"``.

.. req:: Blocks of samples
   :id: REQ_PROTO_SAMPLES
   :status: implemented
   :component: proto
   :satisfies: STK_LIVE_OBSERVE, STK_DEVICE_IF
   :rationale: A target that samples fast sends a block of samples of one trace
      without repeating the path and timestamp of each.

   A ``Samples`` message shall carry the path of one trace, a list of values
   and either a first timestamp and a period, sample ``i`` being at the
   timestamp plus ``i`` times the period, or a list of timestamps of the same
   length as the values. Exactly one of the period and the list of timestamps
   shall be present.

.. req:: Blocks give the same traces
   :id: REQ_PROTO_SAMPLES_APPLY
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall give the same traces for a ``Samples`` message as for the
   equivalent ``Sample`` messages, one for each value. blip shall reject a
   malformed block, with an error notice, and drop only that block, keeping
   the other messages of the frame.
