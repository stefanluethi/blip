Concepts
========

The words used throughout blip, the outpost and sonde, and what they mean for
what you see.

Target, outpost and connection
------------------------------

Target
   The device or program being traced: firmware on a microcontroller, a
   simulator, a unit test.

Outpost
   A service next to the target that serves blip over a WebSocket. It
   translates what the target sends into blip messages, using a *protocol*,
   and carries out the actions blip invokes. See :doc:`outpost`.

Connection
   A WebSocket address in blip's **Connections** panel, with a name, e.g.
   ``outpost`` for ``ws://127.0.0.1:3000/ws/outpost``. blip can be connected
   to several outposts at once; their data is shown together. A target that
   speaks the blip protocol itself over WebSocket can also be connected
   directly.

Messages
--------

Everything a target sends is one of five messages, encoded in CBOR (see
:doc:`/architecture/protocol`):

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Message
     - Meaning
   * - ``Value``
     - A sample of a trace: a timestamp, a path and a value.
   * - ``ValueMetadata``
     - The unit of a trace, e.g. ``rpm``. Sent once or now and then.
   * - ``Log``
     - A log record: timestamp, level, component and message text.
   * - ``Actions``
     - The list of all actions the target can carry out now.
   * - ``ActionResult``
     - The answer to an invocation of an action.

blip sends one message back: ``Invoke``, to carry out an action.

Paths, traces and records
-------------------------

Path
   The name of a trace or action, with levels separated by ``/``, e.g.
   ``sensors/imu`` or ``motor/speed``. blip shows paths as a tree that
   branches at each ``/``. Paths are shown as the outposts send them, without
   the name of the connection.

Trace
   All samples of one path, in the order of their timestamps. Each trace has a
   **kind**, set by its first value other than ``null``:

   * **int** or **float** -- numbers. An integer trace becomes a float trace when
     its first float arrives. Integers are kept exact, also beyond 2\ :sup:`53`.
   * **bool** -- plotted as steps between 0 and 1.
   * **string** -- shown in states and table views, not plotted.
   * **array** -- e.g. ``float[3]`` or ``int[8×8]``: numbers or bools of the same
     shape in every sample. Arrays of up to 16 elements list each element,
     e.g. ``imu.acc[0]``, to plot it on its own. Heatmaps show arrays of one or
     two dimensions as a whole.

   A sample of another kind than its trace is dropped with a warning.

``null``
   A sample without a value, e.g. a failed measurement. Plots show a gap.

Record
   A value that is a map of named fields, e.g. ``{"acc": [...], "valid":
   true}``. blip splits a record into one trace per field, joined to the path
   with ``.``: the record ``imu`` gives the traces ``imu.acc`` and
   ``imu.valid``. The tree lists them below the record as ``.acc`` and
   ``.valid``. A table can also show records as a whole, a row per record.

Unit
   Sent by the target in a ``ValueMetadata`` message and shown next to the
   values, on axes and in tables. blip does not let you set units.

.. figure:: /diagrams/data-model.svg
   :target: ../_images/data-model.svg
   :class: diagram
   :alt: Value messages with records become one trace per field; metadata sets
         units; log messages become log records; the Actions message lists the
         actions.

   What blip makes of the messages.

Log records
-----------

A log record has a timestamp, one of the levels ``TRACE``, ``DEBUG``,
``INFO``, ``WARN`` and ``ERROR``, an optional component, e.g. ``motor``, and a
message. Log views list them, and the timeline at the bottom shows how many of
each level arrived when.

Timestamps
----------

Each target stamps its own samples in nanoseconds: either the time of day
(UTC since 1970) or the time since the target booted. blip shows timestamps
that are too small to be a date (before 2001) as time since boot, e.g.
``0:01:23.456``, and others as local time of day. Targets with different time
bases or clocks are shown next to each other only as far as their clocks
agree.

Views and layout
----------------

View
   A pane in the center that shows traces, logs or actions: a plot, a states
   view, a table, an XY plot, a heatmap, a log view or an actions view. Each
   view has its own selection of traces and its own settings. See
   :doc:`blip/views`.

Selection
   What a view shows is chosen in the **Traces & actions** tree while the view
   is selected. Checking a branch also checks what arrives below it later.

Layout
   The views arranged as a tree of groups: the groups at the top level are
   tabs, the groups below show their children as tabs, stacked vertically or
   side by side. The layout is kept across restarts and can be saved to a
   file. See :doc:`blip/layout`.

Time range, cursor and recording
--------------------------------

Recording
   blip keeps the values and log records only while **Record** is on. A
   recording starts with the data cleared and stops when you stop it; it can
   then be saved to a ``.blip`` file and opened again later. The outpost can
   also record to a file, and sonde writes one from unit tests.

Time range
   All views show the same stretch of time: a **window** of a set length, e.g.
   20 s. In **live** mode it ends at the newest data and moves on with it.
   **Paused**, it stays where it is; **playing**, it moves on at the pace of
   the timestamps, like playing back a video.

Time cursor
   A point in time shown as a line in all plots over time and on the
   timeline. Tables, heatmaps at the cursor and the XY marker show the values
   at its time. Until you set it, it is at the end of the window.

.. figure:: /diagrams/time-range.svg
   :target: ../_images/time-range.svg
   :class: diagram
   :alt: The shown window over the collected data, with the time cursor, when
         live, paused and playing.

   The time range when live, paused and playing.

See :doc:`blip/time` for the controls.

Actions
-------

Action
   Something the target can do on request, e.g. set a motor setpoint, change
   its log level or start a calibration. The outpost announces the actions with
   their parameters; blip shows them as buttons, checkboxes, sliders, fields
   and choices in an actions view.

Invocation
   Carrying out an action once, with arguments. The target accepts it or
   rejects it with a reason, which blip shows; without an answer within five
   seconds it counts as unanswered. blip only invokes an action when you do:
   never on its own, at startup or when it connects again.

Readback
   The trace that shows the current value of what an action sets, e.g. the
   measured speed for a speed setpoint. blip shows it next to the action and
   starts the input from it.

See :doc:`blip/actions`.

Recordings and files
--------------------

Recording file (``.blip``)
   The messages as they were received, in order, compressed. Saved by blip,
   written by the outpost with ``--record`` and by sonde's ``FileSink``.
   Opened by blip, replayed by the outpost.

Layout file (``.json``)
   The views with all their settings, saved and opened from the **File**
   menu.
