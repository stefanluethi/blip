Concepts and data model
=======================

The ideas the parts share, and how blip models what it receives. The user's
view of the same ideas is in :doc:`/usage/concepts`.

Sources, messages and frames
----------------------------

* A **source** is one connection of blip, named in the connections panel. Each
  message blip receives is applied together with the name of its source.
* A **frame** is one binary WebSocket message. It holds one ``Message`` or
  several as a CBOR sequence (RFC 8742), e.g. a batch of sonde passed through
  by the outpost. blip splits frames into messages before anything else, so
  everything later deals with single messages.
* A **message** is the CBOR encoding of a ``proto::Message``. It is what blip
  records, byte for byte, and what a recording file holds.

Paths
-----

Traces and actions are named by paths whose levels are separated by ``/``.
Paths are global: they are not prefixed with the source, so several outposts
sending the same path feed one trace. Record fields are joined to the path of
their record with ``.`` (``imu.acc``), array elements are written with indices
(``imu.acc[0]``), so a field is recognisable as such and the tree can fold a
record.

Traces
------

A trace holds all samples of one leaf path: ascending timestamps and a column
of values.

* **Kinds.** The kind of a trace follows from its first value other than
  ``null``: numbers (integer, float or bool), strings or arrays of numbers
  with a fixed shape. Integers are stored as ``i64``, as ``u64`` once one is
  above ``i64::MAX``, and the column switches to ``f64`` with the first float.
  A sample of an incompatible kind is dropped with a warning, once per trace.
* **Records** are split into their fields, each a trace of its own
  (``fields_start`` marks where the field names begin in the path). The
  traces of a path share its timestamps, so the fields of a record stay
  together, and a field missing from a value keeps its value. Nested records nest the
  paths. A value sent to the path of a field is dropped. The tree and record
  tables put the fields back together where needed, the timeline counts a
  record once.
* **Arrays** are stored row-major in one column per trace, with their shape.
  Views pick single elements or the whole array.
* **Order.** Samples are kept sorted by timestamp; a sample older than the
  newest (e.g. from a second outpost) is moved into place. One older than the
  newest 65 536 values of its path is dropped with a warning, as moving all of
  them for each value of a clock minutes behind would stall blip.
* **Ranges.** Each trace keeps the lowest and highest finite value (per
  element for arrays), so plots fit all data without scanning it.
* **Units** come from ``ValueMetadata`` and are kept by path, also before the
  first sample.

.. figure:: /diagrams/data-model.svg
   :target: ../_images/data-model.svg
   :class: diagram
   :alt: Value messages with records become traces per field; metadata sets
         units; log messages become log records; the Actions message lists the
         actions.

   From messages to the model.

Logs
----

Log records are kept, all of them, in the order of their timestamps with their
level, component and message. A record with the timestamp 0 gets the newest time of the data
received so far (or the wall clock if there is none), so it lines up with the
samples of its target. Levels above 4 count as errors, and a record without a
component shows the component ``target``.

Time
----

Timestamps are nanoseconds in the time domain of the target: UTC or uptime.
blip guesses which from the value (before 2001 means uptime) and shows the axes
accordingly. All views share one **time range** -- a window length and an end
that is either the newest data (live) or a fixed time (paused, or moving at the
pace of the timestamps while playing) -- and one **time cursor**. They count
the time from the first timestamp received, so nanoseconds stay exact also at
Unix times.

.. figure:: /diagrams/time-states.svg
   :target: ../_images/time-states.svg
   :class: diagram
   :alt: Live, paused and playing states of the time range and the inputs
         that move between them.

   States of the time range.

Recording versus receiving
--------------------------

Receiving and recording are separate: blip is always connected to what it is
connected to, but keeps samples (values and log records) only while recording.
What a target sends only once or seldom -- its units and its actions -- is
kept regardless, as the latest **description** per source and path. Clearing
the data, or starting a recording, re-applies these descriptions, so a new
recording starts with them and a saved recording is complete on its own.

Actions
-------

* A source announces **all** its actions in one ``Actions`` message; a new
  list replaces the old one of that source. Several sources may announce an
  action at the same path; blip keeps each, and an actions view chooses which
  source it sends to.
* An **invocation** carries an id unique within the connection. blip tracks
  each pending invocation per (source, path) until its ``ActionResult``
  arrives or five seconds pass.
* The outpost answers invocations itself, after the protocol's action
  returned or raised; ``ActionResult`` messages coming from a target or a
  replay are dropped by the outpost, as their ids belong to another
  connection.
* A **readback** is metadata only: the path of a trace that shows the state
  the action sets. blip shows it and lets toggles and choices follow it, but
  never infers success from it.

.. figure:: /diagrams/action-sequence.svg
   :target: ../_images/action-sequence.svg
   :class: diagram
   :alt: Sequence of announcing, invoking, answering and reading back an
         action.

   Announcing, invoking and reading back an action.

Layouts
-------

The layout is a tree of groups (tabs, vertical, horizontal) with views as
leaves. Each view holds its own selection of paths, the branch keys it checked
as **wildcards** (expanded to new paths whenever the paths of the model
change), its number formats and its type-specific settings. The layout is
saved apart from the app state in a versioned JSON format; older versions are
migrated when loaded.
