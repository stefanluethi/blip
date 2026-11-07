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
   level, component and message. The date of the timestamp and the component
   can be shown or hidden.

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
   time of the time cursor, also as a row per record with a column per field.

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

.. req:: Show the distribution of values
   :id: REQ_VIS_HIST
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall offer a histogram view, which counts the samples of each
   checked trace in bins of their values over the shown time range, and
   counts anew as the range is panned, zoomed or moves on while live. The
   user shall be able to check integer and float traces and the elements of
   arrays of up to 16 elements, but not bool and string traces. Null samples
   shall not be counted. The traces shall be drawn over each other on one
   shared set of bins, each in its colour, as a translucent area under an
   outline in the full colour. The legend shall show a dot and the name of
   each trace, without a value, in the title row of the view, or in its top
   right corner without a title row. The value axis, the bin width, the bin
   edges in the tooltip and the legend shall show the unit of the traces
   only if all checked traces share one. The bin edges and the values on the
   axis shall be written in the number format of the traces if all have the
   same one, else in the default format.

.. req:: Histogram bins
   :id: REQ_VIS_HIST_BINS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall let the user choose the bins of a histogram: automatic, a
   fixed number of bins, or a fixed bin width in the unit of the traces.
   Automatic bins shall be about the square root of the number of samples,
   at least 8 and at most 64, with a width rounded to 1, 2 or 5 times a power
   of ten, over the values shown with a margin of 5 % on each side, and the
   width shall change only once the ideal width is clearly past a step, so
   it does not flip between two widths while live. The edges of automatic
   bins and of bins of a fixed width shall lie on multiples of the bin
   width, so the bars do not shift as the shown time range moves; a fixed
   number of bins shall split the range of the values evenly. Bins of a
   whole-number width over integer traces shall have their edges half a unit
   below the multiples of the width instead, so no integer lies on an edge
   and each bin holds the same number of integers; with a width of 1, the
   bar of a value runs from half a unit below it to half a unit above it.
   The user
   shall be able to fix the minimum and maximum of the values binned;
   samples outside a fixed range shall not be binned but counted, and their
   counts shown faintly at the end they lie beyond, such as "‹ 232" and
   "36 ›". Infinite samples shall not be binned but counted the same way at
   their end, also when the range is not fixed.

.. req:: Histogram counts axis
   :id: REQ_VIS_HIST_COUNTS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   The counts axis of a histogram shall show the samples of each trace in a
   bin as a percentage of all samples of that trace in the shown time range,
   those outside the bins included, so traces of different sample rates can
   be compared. It shall fit the tallest bin in view, and the user shall be
   able to switch it between a linear and a logarithmic scale. The
   logarithmic scale shall have a tick at each power of ten and start at
   least half a decade below the percentage of one sample of the trace with
   the most samples, rounded down to half a decade, so a bin of one sample
   shows as a short bar and an empty bin shows nothing.

.. req:: Histogram mean and spread
   :id: REQ_VIS_HIST_MARKERS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall let the user show, for all traces of a histogram at once,
   no markers (the default), the mean, or the mean and the population
   standard deviation σ (divided by the number of samples, as in the plot
   statistics) of the samples binned, counting each sample once whatever
   time it lasts. They shall be shown in a strip at the far end of the
   counts axis, clear of the bars, with a row per trace: a dot at the mean
   and a whisker from the mean −σ to the mean +σ, in the colour of the
   trace.

.. req:: Histogram orientation
   :id: REQ_VIS_HIST_ORIENT
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall let the user show a histogram upright, with the values along
   the horizontal axis and the bars growing up, or sideways, with the values
   along the vertical axis and the bars growing to the side. The value axis
   of a sideways histogram shall be on the left, as in a plot, or on the
   right, as the user chooses.

.. req:: Histogram linked to a plot
   :id: REQ_VIS_HIST_LINK
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: A histogram beside a plot shows the distribution of the
      plot's traces over the plot's y range; sideways, at the same heights as
      the plot. Taking the plot's traces saves checking them twice.

   blip shall let the user link a histogram, upright or sideways, to a plot
   view of the layout, or to none. While linked, the histogram shall show
   the traces the plot shows, with the plot's number formats, as they
   change, and the tree of the traces shall show them checked but greyed,
   with a hint naming the plot; its number formats shall be greyed too.
   Unlinked, the histogram shall keep the traces it showed last. While
   linked, the value axis of the histogram shall show the y range the plot
   shows, as it is zoomed, panned or fixed, and that range shall act as a
   fixed range: the bins, automatic, a fixed number or a fixed width, shall
   cover it, and samples outside it shall be counted and shown faintly at the
   end they lie beyond. The histogram's own fixed range, zooming and panning
   of the values shall be off while linked, also while the plot shows no y
   range yet. Deleting the plot shall drop the link.

.. req:: Histogram interaction
   :id: REQ_VIS_HIST_HOVER
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   Hovering over a bin of a histogram shall highlight the bin and show its
   range of values, with the unit the traces share, and for each trace its
   percentage of the samples of the trace and its count of them, such as
   "12.5 %" and "42 of 336". The
   mouse wheel shall zoom, and dragging shall pan, the value axis only,
   magnifying the bins without binning anew, and a double click shall fit it
   again. None of them shall change the shown time range or the time cursor.

.. req:: Histogram without data
   :id: REQ_VIS_HIST_EMPTY
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   A histogram without checked traces shall show a hint to check traces in
   the tree of the traces. A histogram whose traces have no samples in the
   shown time range shall show empty axes and "No samples in the shown
   window". A histogram whose samples all have the same value shall show one
   bar centred on that value.

.. req:: Histogram settings saved
   :id: REQ_VIS_HIST_SAVED
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE, STK_REVIEW

   blip shall save the checked traces and all settings of a histogram, its
   bins, range, counts scale, markers, orientation, value axis side and the
   plot it is linked to, with the view in the layout of the preset, and restore
   them when the layout is loaded.

.. req:: Time cursor
   :id: REQ_VIS_CURSOR
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall show one time cursor in all views over time, which the user can
   set and drag, and which stays at the end of the shown time range until set.

.. req:: Record, pause and play
   :id: REQ_VIS_PLAYBACK
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE, STK_REVIEW

   blip shall keep the received values and log records only while recording,
   which the user switches on and off without closing the connections. A
   recording shall start only while a connection is open, and starting it
   shall clear the data received before. blip shall let the user pause the
   shown time range and play it from there at the pace of the timestamps
   while recording goes on.

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

Control
-------

.. req:: Announce actions
   :id: REQ_CTRL_ANNOUNCE
   :status: implemented
   :component: proto, outpost
   :satisfies: STK_CONTROL

   The target shall announce the actions it can carry out with their
   parameters (bool, integer, float, choice or string, with bounds, unit and
   default), an optional readback trace, whether they need confirmation and
   whether they are enabled. Each announcement shall replace the previous one,
   and blip clients connecting later shall get the latest.

.. req:: Invoke actions
   :id: REQ_CTRL_ACTION
   :status: implemented
   :component: blip, outpost
   :satisfies: STK_CONTROL

   blip shall send invocations of announced actions with their arguments to
   the connected target. The outpost shall check the arguments against the
   parameters and carry out the invocations of an action one after another in
   the order they arrived.

.. req:: Action feedback
   :id: REQ_CTRL_FEEDBACK
   :status: implemented
   :component: blip, outpost
   :satisfies: STK_CONTROL

   The target shall answer each invocation to the client that sent it with
   whether it carried it out or the reason it rejected it. blip shall show
   whether the latest invocation of an action is pending, accepted, rejected
   with the reason, or unanswered after a timeout. The outpost shall not
   send log records about invocations.

.. req:: Actions view
   :id: REQ_CTRL_VIEW
   :status: implemented
   :component: blip
   :satisfies: STK_CONTROL

   blip shall offer a view of actions placed with the layout, showing buttons,
   checkboxes, choices, sliders, number and text fields by the kinds of their
   parameters, and the value of their readback traces. Sliders shall send
   while dragged at a limited rate, actions with several parameters shall be
   sent with a separate Send button. The user shall choose the actions a view
   shows in the tree of the traces, where actions are marked as such, per
   view the connection the actions are sent to, and per action whether it has
   the readback trace announced.

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

   blip shall persist the presets, each with the layout of the views and the
   connections, and which preset is active across restarts.

.. req:: Presets
   :id: REQ_PRESETS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall keep named presets, each with the layout of the views and the
   connections, show those of the active preset, and switch between them
   while not recording. Changes of the layout and the connections shall apply
   to the active preset.

Files
-----

.. req:: Save and open recordings
   :id: REQ_FILE_RECORDING
   :status: implemented
   :component: blip
   :satisfies: STK_REVIEW

   blip shall save the messages received since the data was last cleared to
   a recording file, and open recording files, showing their data as if it
   had been received, in place of all data, traces and actions before.

.. req:: Export and import presets
   :id: REQ_FILE_PRESETS
   :status: implemented
   :component: blip
   :satisfies: STK_REVIEW

   blip shall export the presets the user chooses to one JSON file, and
   import the presets the user chooses from a file exported by the same or an
   older version of blip, replacing the presets of the same names.

Outpost
-------

.. req:: Serial outpost
   :id: REQ_OUTPOST_SERIAL
   :status: approved
   :component: outpost
   :satisfies: STK_DEVICE_IF

   The outpost shall forward COBS-framed CBOR messages between a serial port
   and WebSocket clients.

.. req:: Library for user scripts
   :id: REQ_OUTPOST_PROTOCOL
   :status: implemented
   :component: outpost
   :satisfies: STK_DEVICE_IF, STK_CONTROL

   The outpost shall be a Python library that a script of the user, which
   talks to the target, calls to send values, units and log records to blip
   from any thread, without blocking. The script shall declare its actions as
   Python functions whose parameters are taken from their type hints, and may
   change them at runtime.

.. req:: Sending many at once
   :id: REQ_OUTPOST_BATCH
   :status: implemented
   :component: outpost
   :satisfies: STK_DEVICE_IF, STK_LIVE_OBSERVE

   Each call of the outpost to send shall carry all of a message, its full
   path and, for a log record, its level, path and component, so a script
   forwards what a target sends without declaring anything first. The
   outpost shall send many samples of one trace at once, at their timestamps
   or at a fixed period, and hand all that a thread sends in a block over at
   once, with one timestamp for what is sent without one.

.. req:: Tracers for Python programs
   :id: REQ_OUTPOST_TRACERS
   :status: implemented
   :component: outpost
   :satisfies: STK_DEVICE_IF, STK_LIVE_OBSERVE

   A Python program that traces itself shall get tracers by name, as it gets
   loggers: the same name gives the same tracer anywhere in the program, its
   traces are below its name, and its log records are at its name and of its
   component, with the levels as methods. A dataclass shall be sent as a
   record, with the units its fields declare, and the records of Python's
   ``logging`` shall reach blip with the name of the logger as the component.

.. req:: Socket outpost
   :id: REQ_OUTPOST_SOCKET
   :status: implemented
   :component: outpost
   :satisfies: STK_DEVICE_IF

   The outpost shall forward messages between a TCP or UDP endpoint and
   WebSocket clients, and forward invocations to the endpoint. The endpoint
   may send several messages at once as a CBOR sequence (RFC 8742): a UDP
   datagram holds complete messages, a TCP stream may split them anywhere.
   The outpost shall forward each complete message and drop data that is no
   CBOR without stopping.

.. req:: Simulator
   :id: REQ_OUTPOST_SIM
   :status: implemented
   :component: outpost
   :satisfies: STK_DEVICE_IF, STK_CONTROL

   The outpost shall come with a simulator, an example script that streams
   synthetic CPU load and sensor values at different sampling rates, and log
   records of all severity levels, without hardware, and offers actions that
   change what it streams.

.. req:: Record messages
   :id: REQ_OUTPOST_RECORD
   :status: implemented
   :component: outpost
   :satisfies: STK_REVIEW

   The outpost shall record the messages it sends to blip to a recording file
   that blip opens.

.. req:: Replay recordings
   :id: REQ_OUTPOST_REPLAY
   :status: implemented
   :component: outpost
   :satisfies: STK_REVIEW, STK_DEVICE_IF

   The outpost shall send the messages of a recording file to blip unchanged,
   except that recorded actions are disabled, at the pace of their timestamps
   or faster.
