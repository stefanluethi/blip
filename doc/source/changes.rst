Release notes
=============

What changed in each release of blip, the outpost and sonde, and what to do
when upgrading. The newest release is at the top. Each release lists what is
new, what changed, what was removed, and under **Upgrading** what you have to
change, e.g. in scripts, schemas, layout or recording files.

0.1.0 (not released yet)
------------------------

The first release.

blip
   * Plots, states, tables, XY plots, distributions, heatmaps, logs and actions
     views, arranged in tabs and groups.
   * Distributions of how the values spread over the shown time range, as
     histograms in percent of the samples of each trace, as box plots with
     median, mean, quartiles, whiskers and outliers, or both, upright or
     sideways; a distribution linked to a plot shows the plot's traces over
     its y range.
   * Presets: layouts with their connections, to switch between, exported to
     and imported from JSON files.
   * One time range for all views: live, paused or playing, with a time cursor
     and a timeline of all data.
   * Records with fields, arrays of one and two dimensions, units, display
     formats, and stats of the traces below plots.
   * Recording, saved compressed to ``.blip`` files and opened again.
   * Actions with buttons, checkboxes, sliders, fields and choices, carried
     out only while recording.
   * Connections to several outposts at once; in the browser and native.
   * The code of blip is split into the application ``blip`` and the library
     ``blip-kit`` it is built on, with the store, the widgets and the
     workbench; the crate of the wire types is now named ``blip-proto``. blip
     works as before, see :doc:`/blip/internals`.
   * Internal: the received data is the ``MemoryStore`` (was ``Model``), which
     the widgets read through the new ``Store`` trait, asking for the samples
     they show in a ``Window``. A ``Session`` holds the store, the invoker and
     the sources, and the new ``Source`` trait, with the ``WebSocket`` source,
     tells where messages come from. The presets are kept through a ``Storage``
     trait of the workbench, not eframe's. Saved presets and recordings are
     unchanged.
   * Internal: the store takes typed input, not CBOR. The ``Ingest`` trait with
     its ``Input`` is the entry of a store, and the value of a sample is a
     ``Value``, written by anything that is an ``AsValue``. A ``Source`` hands
     what it has to an ``Inbox``: bytes in the blip protocol with ``frame``, or
     typed samples, logs, metadata and actions, so a source whose data never was
     CBOR does not encode it. The ``Session`` works with any store that takes
     input and holds the ``Recorder`` that writes the recording file. The CBOR
     reader and the parser are private. The rate at which the store takes in
     samples and the frame time of a plot are as before.
   * Internal: the connections of blip are sources of the session, so every
     message reaches the store by one path, the inbox of the session. The
     ``Connections`` add a ``WebSocket`` per address to the session and read
     what happened from it. A connection error is now one of the session's
     notices, in the order it happened; the texts are as before. A request to a
     name with several open sources, e.g. two connections without a name that
     both announce actions, goes to the one that announced its actions last;
     before, to the first.
   * Saved recordings are written again from what blip received: numbers in
     their shortest form, a block of samples as a block, the old names
     ``Value`` and ``ValueMetadata`` as ``Sample`` and ``TraceMetadata``. Messages
     blip cannot read are kept as they arrived, and a recording that was opened
     and is saved again is written unchanged. The format and its version are the
     same, and old files open as before.
   * CBOR the protocol does not allow inside a value (a byte string, a tag, an
     indefinite-length array) now makes the whole message unreadable, where only
     that value was dropped before.
   * The protocol has a schema in CDDL, ``proto/blip.cddl``, shown in
     :doc:`/reference/protocol`; what ``blip-proto``, the outpost and sonde
     write is checked against it. The reference now says how a message is
     encoded: a map of definite length with one entry.
   * Empty views, panels and lists show their hint in one style. Plot and States
     now say *Check the traces to show in the tree on the left* over their
     empty plot.
   * Hover tooltips of the plot, states, XY and heatmap views and of the
     distributions look alike: a framed card with a header and right-aligned
     values. A trace without a value at the pointer is now listed with a faint
     *—*. XY plots and the array line of heatmaps get the same framed tooltip.
   * Log records are told apart by their path, by convention ``log/<part>``,
     e.g. ``log/imu``, and have no component. A record without a path is at
     ``log/default``. The log table has a **Path** column, relative to the
     folder the paths share, and a **Path** filter in place of the component
     filter; checking the ``log`` folder shows all log paths below it.
   * Protocol version 1: a target says who it is in a ``Hello`` (name, clock,
     sender) and what its traces mean in a ``TraceMetadata`` (unit,
     description, labels for integer values, expected range), and may send
     blocks of samples in a ``Samples``. The connections show the target
     name, the sender and the clock; a newer protocol than blip reads gives a
     warning and an amber dot. The clock of the ``Hello`` sets the time base
     instead of the guess; when connections declare different clocks, blip
     warns once and keeps the time of day.
   * Labels: the new number format, the default for a trace with labels, writes
     *Run* for the integer 2 in States, tables, tooltips and legends. Tooltips
     show the number faint beside a label, and the description of the trace
     under the pointer. The properties section **Traces** (was **Number
     format**) and the hover of the traces show the description, the expected
     range and the labels.
   * **Fit to** *Expected* or *Values* on the Y axis of plots and the axes of XY
     views: the axis fits the range a trace declares, grown by values beyond
     it. The colour scale *Auto* of a heatmap is that range.
   * A malformed block of samples is dropped alone, with an error notice.
   * Internal: shared helpers for the timestamp searches (``store/sorted``),
     the drawing of the plot views (``view/plotting``), theme tokens, fields
     and property rows.
   * Keyboard shortcuts: :kbd:`R` records, :kbd:`Space` plays and pauses,
     :kbd:`L` goes live, :kbd:`Escape` releases the time cursor,
     :kbd:`Ctrl+F` searches the traces, :kbd:`F2` renames and :kbd:`Delete`
     deletes the selected view, besides :kbd:`Ctrl+O` and :kbd:`Ctrl+S`. They
     are listed under **Help › Keyboard shortcuts** and in
     :doc:`/blip/controls`.
   * An unsaved recording is not dropped without asking: *Discard the unsaved
     recording?* before Record, **Clear recording**, opening or dropping a
     recording and quitting; the web page warns on reload. Connecting keeps
     it. Opening a file keeps the connections and a running recording until
     the file is read, and Record cancels the opening. While connected and not
     recording, the Record dot is amber and the timeline says *Press Record to
     collect data*. **File › Clear data** is **Clear recording**; saving
     says *Saved <name>*.
   * Deleting a view, tab or group, and **View › Clear layout**, can be
     undone with the **Undo** button of the notice or **View › Undo delete**.
     Tabs are renamed with a double click and deleted with a **✕**; the keys
     :kbd:`F2` and :kbd:`Delete` act on the selected view.
   * Notices have an info level for what went well, errors stay until
     closed, and a notice can have a button.
   * Connections: a refused edit keeps the field open with the reason, an
     address without a scheme gets ``ws://``, deleting asks in the row, and a
     connection closed by the other side is *Connection lost* with a red dot,
     also in the chip. Switching, adding and duplicating presets keep the open
     connections the new preset also has. Importing presets says *Import and
     replace* when it replaces one.
   * The time panel: **Live** also in an opened recording and while the cursor
     is set, **Play** disabled without data, a window length field with a menu
     of round lengths that reads ``us``, ``ms``, ``s``, ``min``, ``h`` and
     ``d`` and marks what it cannot read, scrubbing the ruler moves the
     window, gaps say *No data for* their duration, and *Paused, N s behind*
     while recording.
   * The traces tree is *Traces, actions and logs*: a **checked only** button
     and a count of checked traces hidden by the search, **Expand all** and
     **Collapse all**, :kbd:`Alt`-click on a caret for the subtree, arrow keys,
     a faint **+** on folders that also check what arrives later, and radio
     buttons in the heatmap.
   * One range control in all views: **Fit to**, then **Min** and **Max**,
     empty to fit; wrong entries are outlined in the error colour with the
     reason on hover. A zoomed plot shows its range as the hints, with a
     **Zoomed** row and a Fit button. Double-clicking a plot fits the y axis
     and leaves the time and the cursor.
   * Legends in the corners of plots cannot hide traces; the legend in the
     title row drops the unit, then cuts a name, when the room is short.
     Hovering a legend value or a stat chip shows all digits.
   * XY: a legend and **Placement**, **Width**, and sections **X axis** and
     **Y axis** with **Min**, **Max** and **Label**, a row per curve, no hover
     rulers. The heatmap at the cursor has a **Y axis** section and **Lines**,
     and its grid fills its card.
   * Logs: scrolling up pauses following, a click on an entry sets the time
     cursor, messages are one line, and right-click copies the message or the
     row. Actions: a typed number is sent with :kbd:`Enter` only, and **Run**
     waits for the target's answer. Tables: right-click copies a value or a
     row, and **Show empty traces**.
   * The native window starts at 1280 × 800, the browser without WebGL says
     so, and **Help › Documentation** opens a new tab.
   * Internal: ``Toasts::info``, ``notice``, ``with_action``, ``take_actions``
     and ``dismiss``, with a ``toasts::Level`` of their own;
     ``files::save`` returns the name saved (``Result<Option<String>, _>``)
     and ``files::save_label``; ``Session::is_unsaved``, ``mark_saved`` and
     ``waiting_for_record``; ``Connections::status``, ``Status`` and
     ``clear_failures``; ``Layout::undo_delete``, ``can_undo_delete``,
     ``take_deleted``, ``undo_number``, ``clear``, ``delete_selected`` and
     ``rename_selected``; ``TimeRange::release_cursor``, ``toggle_play`` and
     ``set_connected``, and ``TimeRange`` is ``Clone``;
     ``LogTable::ui`` takes ``&mut TimeRange``; ``Ends`` and
     ``range_rows(ui, &mut Ends, fitted)`` replace the fixed ranges of the XY
     view, the heatmap and the distribution; ``trace_selection::search_ui``
     and ``show`` take a ``TreeFilter``, and ``request_search_focus`` focuses
     the search.

blip-kit
   * The library blip is built on, to build your own tracing UI in egui, in
     the browser and natively, see :doc:`/library/index`.
   * The ``store`` has no GUI dependency: the traces, logs and actions
     received, the ``Store`` trait the widgets read through, the in-memory
     ``MemoryStore``, the ``Source`` trait with a ``WebSocket`` source, the
     ``Session`` that receives and sends the invocations each frame, recording
     files and the invoker of actions.
   * The ``widgets``: the views (plot, states, table, XY plot, distribution,
     heatmap, log table and actions), made in code with ``view::showing``, the
     time controls and the timeline, and the theme. A table takes columns of
     the application, e.g. a button that invokes an action.
   * Views of your own: register a type with ``custom::register``, to add it to
     the layout like the others; presets save it by its name and keep views of
     types that are not registered.
   * The ``workbench``: the layout the user arranges the views in, the presets,
     the connections, the toasts and the file dialogs, which is what blip is.
   * The example ``dashboard`` is a whole application in code, with a simulated
     target in the application itself.
   * Not yet: a loading state in the widgets, publication on crates.io, a store
     in a database.

outpost
   * The Python library ``blip_outpost`` for scripts that connect a target:
     values, records, log records, actions, recording.
   * The ``blip-outpost`` command: pass through targets that speak the blip
     protocol over TCP or UDP, translate protobuf targets from an annotated
     schema, replay recordings.
   * Examples: a simulator, the smallest script, and a stress test.
   * ``metadata(path, unit=None, *, description, labels, value_range)`` on the
     outpost, the tracers and the module, merging per path; ``Description``,
     ``Labels`` and ``Range`` annotations on the fields of dataclass records.
   * ``Outpost(name=…, clock=…)``, also in ``start`` and ``serve``, for the
     ``Hello`` sent to each blip first, with the hostname and ``"epoch"`` by
     default. A passthrough, a replay and a protobuf link send none of their own.
   * ``series`` sends one ``Samples`` message instead of a ``Value`` for each
     sample. The simulator sends metadata and a block of vibration samples.

sonde
   * The C++17 library for firmware and unit tests: values, records, arrays and
     log records in batches, sent by the firmware or written to recording
     files by a sink. A tracer's log records are at its name.
   * Version 0.2.0: every queue sends a ``Hello`` first (``Hello{name,
     TimeBase}`` in the constructor, else the version and the sender only);
     ``metadata()`` with ``Label``, ``Range`` and ``Metadata``; ``samples()``
     with a ``Period`` or timestamps; recordings of version 3.

Upgrading
   Log records have no component any more. In sonde, ``Queue::log`` and
   ``logf`` and ``Tracer`` take no component argument; log at a path instead,
   e.g. ``log/imu``. In the outpost, ``log()``, ``get_tracer()`` and ``Tracer``
   take no ``component``; pass ``path="log/imu"``. ``LogHandler(path="")``
   sends the logger ``app.motor`` to ``log/app/motor``, below ``path`` if
   given. Old recordings still load, their component is dropped. A log view
   that showed only the old path ``log`` shows nothing until you check the
   ``log`` folder again; saved component filters of layouts are dropped.

   Presets replace the layout files of the development versions: blip no
   longer opens layout files, and the layout and connections it kept are
   dropped on the first start. Arrange the views again and export them as a
   preset, see :doc:`/blip/presets`.

   The wire messages ``Value`` and ``ValueMetadata`` are now ``Sample`` and
   ``TraceMetadata``. blip and the outpost still read the old names, in
   recordings and from targets, and the outpost forwards them unchanged. A
   script that builds messages by hand with ``Message.create_value`` and
   ``create_metadata`` needs no change. A tool of your own that reads the CBOR
   of recordings or of the WebSocket must accept the new names too.

   Recording files are version 3 now (the layout is that of before). blip and
   the outpost read versions 1 to 3; older blips and outposts refuse version 3.
   Presets files and the presets blip keeps are version 2 now, adding the
   number format ``Labels`` and ``fit``: an older blip refuses them (*holds
   presets of a newer version of blip*), a version 1 file loads without a
   notice. A trace with labels is shown with its labels unless a layout chose
   another format; choose *Default* in **Traces** to see the numbers.

   The ranges of the XY view, the heatmap and the distribution are a min and a
   max, each empty to fit, in place of a fixed range or none. Saved presets
   load to the same picture; presets saved now write ``min`` and ``max``, which
   a blip of before this change may not read. Code that calls ``files::save``, ``LogTable::ui`` or
   ``trace_selection::show`` and ``search_ui`` needs the new signatures, see
   above.

   Without a ``Hello`` blip guesses the time base as before, so old targets
   need no change; add ``clock`` to the ``Hello`` of your target to make it
   certain.

   Recordings saved by blip are equivalent to what it received, not byte for
   byte the same: numbers are written in their shortest exact form, the old
   names ``Value`` and ``ValueMetadata`` as ``Sample`` and ``TraceMetadata``, and
   fields blip does not know in a message it can read are not kept. A tool that
   compares the file with the bytes a target sent must compare the messages
   instead. Nothing to do for old files: they open as before, and an opened file
   that is saved again is unchanged.

   Applications built on ``blip-kit`` (not published yet, so there is no
   compatibility layer):

   * A ``Source`` no longer returns events. ``poll(&mut self)`` is now
     ``poll(&mut self, inbox: &mut Inbox<'_>)``: tell ``inbox.opened()``,
     ``inbox.closed()`` or ``inbox.failed(reason)``, and hand over data with
     ``inbox.frame(bytes)`` for blip-protocol bytes or the typed calls
     ``sample``, ``samples``, ``log``, ``metadata``, ``actions``, ``hello`` and
     ``action_result``. ``Event``, ``Event::Frame`` and ``Event::message`` are
     gone; ``inbox.message(&message)`` takes a ``proto::Message`` built by the
     source. A source holds no parser.
   * ``MemoryStore::receive`` and ``receive_unrecorded``, ``Session::receive``
     and ``Session::receive_frame`` are gone. Use
     ``Session::inbox(source, now)`` and its calls, or ``Ingest::ingest(source,
     Input)`` on a store directly. The ``Session`` decides what is kept while
     not recording, not the application.
   * The recording is in the session: ``session.recorder`` (type ``Recorder``,
     was ``Recording``) in place of ``store.recording``. ``Session::start_fresh``
     takes the store and a recorder; ``FileReader::read`` takes a
     ``&mut dyn Ingest`` and ``finish`` gives the recorder and what was loaded.
   * ``Session<S>`` works with any store that is ``Ingest + Store``:
     ``Session::new(store)``. A store of your own implements ``Ingest``.
   * ``store::cbor``, ``store::decode`` (now the private ``protocol``),
     ``store::growing`` and ``store::sorted`` are no longer public. What a view
     needs of ``sorted`` are the methods ``index_at``, ``closest_index`` and
     ``within`` of ``Series`` and ``Trace``.
   * ``Session::add_source(name, source)`` returns a ``SourceId`` and no longer
     replaces a source of the same name. ``remove_source`` takes the id, not
     the name. ``update(now)`` returns an ``Update`` (``messages``,
     ``started_fresh``). New are ``set_label(id, label)``, which the notices
     use, and ``source_events()``, what the sources told in the last update.
     ``poll_source`` and ``Polled`` are private; add the source to the session
     and call ``update``. ``Received`` is gone.
   * In the workbench, ``Connections::receive``, ``send`` and ``take_errors``
     are gone: the session polls the sockets and sends the requests, and the
     errors are in ``Session::take_notices``. Call
     ``Connections::observe(&session)`` after each ``Session::update``.
     ``ui``, ``set_saved``, ``disconnect_all`` and ``connect_first`` take the
     session. ``Opening::finish(&mut session)`` puts the opened recording in
     the session.
