Concepts
========

The ideas blip, the outpost and sonde share: what a target sends, how it is
named and timed, and what an action and a recording are. What blip does with
them on the screen is in :doc:`/blip/index`.

Target, outpost and connection
------------------------------

Target
   The device or program being traced: firmware on a microcontroller, a
   simulator, a unit test.

Outpost
   A service next to the target that serves blip over a WebSocket. It
   translates what the target sends into blip messages and carries out the
   actions blip invokes. See :doc:`/outpost/index`.

Connection
   A WebSocket address blip connects to, with a name, e.g. ``outpost`` for
   ``ws://127.0.0.1:3000/ws/outpost``. blip can be connected to several
   outposts at once; their data is shown together. See
   :doc:`/blip/connecting`.

Messages
--------

Everything a target sends is one of five messages, encoded in CBOR:

.. list-table::
   :header-rows: 1
   :widths: 20 40 40

   * - Message
     - Meaning
     - Sent by the outpost with / by sonde with
   * - ``Value``
     - A sample of a trace: a timestamp, a path and a value.
     - ``value`` / ``value``
   * - ``ValueMetadata``
     - The unit of a trace, e.g. ``rpm``. Sent once or now and then.
     - ``unit`` / ``unit``
   * - ``Log``
     - A log record: timestamp, path, level and message text.
     - ``log`` / ``log``, ``logf``
   * - ``Actions``
     - The list of all actions the target can carry out now.
     - the ``@action`` functions / --
   * - ``ActionResult``
     - The answer to an invocation of an action.
     - returning or raising in an action / --

blip sends one message back: ``Invoke``, to carry out an action. Several
messages may travel in one WebSocket frame, e.g. a **batch** of sonde, one
after another. The encoding is in :doc:`/reference/protocol`.

Paths, traces and records
-------------------------

Path
   The name of a trace, a log or an action, with levels separated by ``/``,
   e.g. ``sensors/imu`` or ``motor/speed``. Paths are global: blip shows them
   as the outposts send them, without the name of the connection, so two
   outposts sending the same path feed one trace.

Trace
   All samples of one path, in the order of their timestamps. Each trace has a
   **kind**, set by its first value other than ``null``:

   * **int** or **float** -- numbers. An integer trace becomes a float trace when
     its first float arrives. Integers are kept exact, also beyond 2\ :sup:`53`.
   * **bool** -- true or false.
   * **string** -- text, e.g. the name of a state.
   * **array** -- e.g. ``float[3]`` or ``int[8×8]``: numbers or bools of the same
     shape in every sample. The elements of an array have paths of their own,
     e.g. ``imu.acc[0]``.

   A sample of another kind than its trace is dropped.

``null``
   A sample without a value, e.g. a failed measurement.

Record
   A value that is a map of named fields, e.g. ``{"acc": [...], "valid":
   true}``. Each field is a trace of its own, its name joined to the path
   with ``.``: the record ``imu`` gives the traces ``imu.acc`` and
   ``imu.valid``. The first record at a path sets its fields.

Unit
   Sent by the target in a ``ValueMetadata`` message, e.g. ``°C``, and shown
   next to the values. Units come from the target only.

.. figure:: /diagrams/data-model.svg
   :target: ../_images/data-model.svg
   :class: diagram
   :alt: Value messages with records become one trace per field; metadata sets
         units; log messages become log records; the Actions message lists the
         actions.

   From messages to traces, log records and actions.

Log records
-----------

A log record has a timestamp, an optional path, one of the levels ``TRACE``,
``DEBUG``, ``INFO``, ``WARN`` and ``ERROR`` and a message.

The path says where a record comes from, like the path of a trace, e.g.
``motor`` or ``robot/arm``; a record without a path is of the target as a
whole. By convention log records are at ``log/<part>``, e.g. ``log/imu``, so
the part of the software that wrote a record is in its path. blip shows a
record without a path at ``log/default``.

Timestamps
----------

Each target stamps its own samples in nanoseconds: either the time of day
(UTC since 1970) or the time since the target booted. A sample sent without a
timestamp gets the time it is sent: by the outpost, the time of the PC, or the
time of the target while the outpost passes one through or translates it; by
sonde, the time of the queue's clock. blip shows timestamps that are too small
to be a date (before 2001) as time since boot, e.g. ``0:01:23.456``, and others
as local time of day. Targets with different time bases or clocks line up only
as far as their clocks agree.

The first sample sets the time base of the data: samples of the other base,
e.g. at the time of day while the target sends its uptime, are dropped, with a
warning, as one time axis cannot show both. Clearing the data starts afresh.
Log records without a timestamp that arrive before any data get the time of
the computer, and move to the first timestamp if it is an uptime.

When a target restarts, its uptime starts over. The outpost shifts the
timestamps of the new run so that they follow the ones before, by the time
that passed in between, and logs a warning at the restart; the samples of the
two runs do not overlap.

Actions
-------

Action
   Something the target can do on request, e.g. set a motor setpoint, change
   its log level or start a calibration. The outpost announces the actions with
   their parameters, each with a type, a range and a unit; blip shows them as
   inputs in an actions view.

Invocation
   Carrying out an action once, with arguments. The target accepts it or
   rejects it with a reason; without an answer within five seconds it counts
   as unanswered. blip only invokes an action when you do: never on its own,
   at startup or when it connects again.

Readback
   The trace that shows the current value of what an action sets, e.g. the
   measured speed for a speed setpoint. It shows the effect; the answer to the
   invocation only says whether the target took it.

.. figure:: /diagrams/action-sequence.svg
   :target: ../_images/action-sequence.svg
   :class: diagram
   :alt: Sequence of announcing, invoking, answering and reading back an
         action.

   Announcing, invoking and reading back an action.

How to offer actions is in :ref:`outpost-actions`, how to carry them out in
:doc:`/blip/actions`.

Recordings
----------

Recording file (``.blip``)
   The messages as they were received, in order, compressed. Saved by blip,
   written by the outpost with ``record`` and by sonde's ``FileSink``. Opened
   by blip as if the messages arrived again, replayed by the outpost at their
   pace. See :doc:`/reference/files`.

blip keeps what arrives only while it records; the outpost and sonde record
whenever they are told to. Invocations are not part of a recording: the log
records of the target show what it did.
