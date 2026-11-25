blip internals
==============

How blip is built, for those who change it. How to use it is in
:doc:`index`; the messages it reads are in :doc:`/reference/protocol`.

blip is an `egui <https://github.com/emilk/egui>`_/eframe application. The
same code compiles to WebAssembly, served by Trunk, and to a native binary. It
is an immediate-mode GUI: each frame, blip applies what arrived and draws the
whole window again from its state.

Crates
------

blip is built from three crates of one workspace. How to use the library in your
own application is in :doc:`/library/index`; this page is about how it is built.

.. list-table::
   :header-rows: 1
   :widths: 22 18 60

   * - Crate
     - Directory
     - Contents
   * - ``blip-proto``
     - ``proto/``
     - The wire types. The other crates use it under the name ``proto``.
   * - ``blip-kit``
     - ``kit/``
     - The library: the ``store`` of what was received, the ``widgets`` that
       show it and the ``workbench``, the layout the user arranges them in.
       It re-exports the wire types as ``blip_kit::proto``.
   * - ``blip``
     - ``blip/``
     - The application: ``main.rs`` and the ``App``, which puts the parts of
       the workbench into a window. It is the reference of an application
       built on ``blip-kit``.

``blip-kit`` is one crate with features, as its parts change together:

.. list-table::
   :header-rows: 1
   :widths: 22 78

   * - Feature
     - What it adds
   * - (none)
     - ``store``: always there, and without a GUI crate among its
       dependencies. ``check.sh`` builds it alone, natively and for the web,
       and fails if ``egui`` or ``eframe`` shows up in its dependencies.
   * - ``websocket`` (default)
     - ``store::websocket``: the WebSocket ``Source``. Needs ``ewebsock``.
   * - ``widgets`` (default)
     - ``widgets``: the theme, the time range with its controls and timeline,
       and the views. Needs egui.
   * - ``workbench``
     - ``workbench``: the layout, the presets, the connections, the toasts
       and the file dialogs. Turns on ``widgets`` and ``websocket``. blip uses
       it.
   * - ``test-util``
     - Helpers for the tests of an application, e.g. selecting a view by its
       name. blip's tests use it.

The library is described for its users in :doc:`/library/index`. The example
``kit/examples/dashboard.rs`` is an application built on ``store`` and
``widgets`` alone, see :doc:`/library/app`.

The modules depend on each other in one direction only: ``workbench`` uses
``widgets`` and ``store``, ``widgets`` uses ``store``, ``store`` uses neither.

Structure
---------

.. figure:: /diagrams/blip-modules.drawio.svg
   :target: ../_images/blip-modules.drawio.svg
   :class: diagram
   :alt: The App holds the State (time range and session), the
         connections, the files, the toasts and the layout with its views; the
         sources of the session exchange binary frames with outposts; all use the blip-proto
         crate.

   Modules of blip and its library. The session holds the store, the invoker
   and the sources, among them the WebSocket sources; the connections add them
   to the session and show their status; the table below lists the modules.

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Module
     - Responsibility
   * - ``blip/src/main.rs``
     - Starts eframe natively or on the canvas of ``index.html`` on the web;
       on the web also saves the state when the page is hidden.
   * - ``blip/src/app.rs``
     - ``App``: the panels of the window, the menu, opening and saving files,
       persistence. ``State``: the time range and the ``Session``, with the
       rules of the session (a recording starts only while connected and
       clears the data; the first connection to open starts afresh, unless
       that drops an unsaved recording). The frame calls ``Session::update``
       and then ``Connections::observe``. The keyboard shortcuts
       (``SHORTCUTS``, listed under Help), the question whether to discard the
       unsaved recording and the "Drop to open" overlay.
   * - ``store``
     - What was received. ``store.rs`` is the ``MemoryStore``, which takes
       typed input and keeps it, and the ``Level`` of the notices for the user.
       Its public parts: ``ingest`` (the ``Ingest`` trait with its ``Input``
       and ``Times``: what puts data into a store), ``value`` (the ``Value`` of
       a sample, ``Rows`` and ``AsValue``), ``traces`` (with ``numbers``, the
       columns of numbers), ``logs``, ``actions`` (the announced actions and
       the ``Invoker``), ``recording`` (the ``Recorder`` that writes what
       arrives to a recording file, and the reader of recording files) and
       ``time_base`` (what the timestamps count from, with ``seconds_since``
       and ``timestamp_at``, re-exported by ``widgets::time``). Its private
       parts: ``cbor`` (the CBOR reader and writer), ``protocol`` (the parser
       of the messages, the only one in front of the store), ``said`` (what
       the parser makes of a message), ``flatten`` (a value into the leaves
       the traces hold) and ``growing`` (the columns); ``sorted`` holds the
       searches of ascending timestamps and is private to the crate.
   * - ``store/query.rs``
     - The ``Store`` trait all widgets read through (as ``&dyn Store``), the
       ``Window`` a widget asks for and the ``Loaded`` answer. See
       :doc:`/library/stores`.
   * - ``store/source.rs``, ``store/websocket.rs``
     - The ``Source`` trait and the ``Inbox`` a source hands its messages to,
       and the ``WebSocket`` source on ``ewebsock``. See
       :doc:`/library/sources`.
   * - ``store/session.rs``
     - The ``Session``: the store (any ``Ingest + Store``, the
       ``MemoryStore`` by default), the ``Recorder``, the ``Invoker``, whether
       recording and the sources, whether the data is an unsaved recording
       (``is_unsaved``, ``mark_saved``), and what happens each frame apart from
       the GUI.
   * - ``widgets/time.rs``, ``widgets/time/timeline.rs``
     - The ``TimeRange`` shared by all views, its controls and the timeline
       with the activity bars.
   * - ``widgets/view``
     - The views and what they share: the ``Viewable`` trait, the
       ``trace_selection`` tree, the ``time_axis`` of the views over time,
       ``plotting``, the drawing pieces the plot views share, and the
       properties widgets. ``view::showing`` makes a view in code.
   * - ``widgets/view/custom.rs``
     - The registry of the view types of an application, ``custom::register``,
       and ``CustomView``, the state of a view of such a type, saved by its
       name.
   * - ``widgets/theme.rs``
     - The colours, fonts and shared widgets of the look. The fonts are in
       ``kit/assets/fonts``.
   * - ``workbench/presets.rs``
     - The ``Presets``: the kept presets and which is active, their menu, the
       dialogs to rename, delete, export and import them, the versioned
       ``PresetsFile`` format and the ``Storage`` trait they are kept in.
   * - ``workbench/connections.rs``
     - The list of connections, opened from the status chip in the menu bar.
       Adds one ``WebSocket`` source per open address to the session, with the
       address as its label, and removes it when the user disconnects. Shows
       the status and the ``Hello`` of each from the events of the last update
       (``Session::source_events``) and keeps the warning of a newer protocol.
       It polls and sends nothing: the session does. ``Connections::status``
       gives the ``Status`` for the chip (connecting, failed or lost, while
       none is open) and ``clear_failures`` drops the failures, e.g. when a
       recording is opened. A refused edit keeps its field open with the
       reason.
   * - ``workbench/layout``
     - The ``Layout``: an ``egui_tiles`` tree of views, the menus to add and
       delete them in the center, the properties panel (``properties.rs``), dragging in
       the center (``drag.rs``), the view picker (``picker.rs``) and the saved form
       (``file.rs``). Deleting a
       view, tab or group, and ``Layout::clear``, keep the layout before in a
       single undo step, see below.
   * - ``workbench/files.rs``, ``workbench/opening.rs``
     - Open and save dialogs (``rfd`` natively, a file input and a download
       on the web) and dropped files; reading a recording a part in each
       frame. ``files::save`` returns the name it saved, or ``None`` if the
       user cancelled; ``files::save_label`` adds the ellipsis of a dialog
       natively.
   * - ``workbench/toasts.rs``
     - The notices in the bottom right. A ``Toast`` has a ``Level`` of its own
       (``Info``, ``Warning``, ``Error``): the first two expire, errors stay
       until closed. ``Toasts::with_action`` adds a button, e.g. *Undo*; its
       clicks come back from ``take_actions`` as the value given, and
       ``dismiss`` closes the offer.

The modules without a crate before them are in ``kit/src``.

The ``blip-proto`` crate holds the wire types. The store takes typed input and
reads no CBOR. CBOR ends at the parser (``store/protocol.rs``) that the inbox of
a source holds. Values and log records, which arrive all the time, it reads with
the store's own CBOR reader (``store/cbor.rs``) in one scan of the frame: it
borrows the strings of the frame and writes a value into a ``Value``, a tape of
items in a buffer that is kept for the next value. The store then flattens it
(``store/flatten.rs``) into its leaves, the fields of records and their arrays of
numbers. The other messages are decoded with ``serde_cbor_2``. The same reader
and a writer next to it serve the recorder.

The frame
---------

eframe calls ``App::logic`` and then ``App::ui`` for each frame. blip repaints
when there is input, and the sockets wake it up when a message arrives, so it
draws as often as data arrives, not more.

.. figure:: /diagrams/blip-frame.drawio.svg
   :target: ../_images/blip-frame.drawio.svg
   :class: diagram
   :alt: logic(): update the session, which polls its sources, parses the
         frames, routes what arrives to the recorder, the store and the invoker
         and updates the invoker; the connections observe the update; the
         toasts; open files. ui(): menu, update the time range, bottom, left and
         right panels, the views, then send the invocations.

   What happens in a frame.

* ``logic`` calls ``Session::update``. It sends the invocations queued since
  the last frame, polls each source, which gets an inbox, and then tells the
  invoker which sources are open. The session routes each message the inbox
  reads, see *Receiving*: to the recorder, the store (while recording, or only
  the latest value of a sample otherwise) and, for answers to invocations, the
  invoker. The first connection to open starts afresh, unless recording or
  the data is an unsaved recording.
  ``update`` returns an ``Update``: how many messages arrived, for the rate,
  and whether the session started afresh, on which blip resets the traces
  panel. ``Connections::observe`` then reads what the sources told in the
  update (``Session::source_events``) to show the status and the ``Hello`` of
  each connection. The errors of connections come out with the notices of the
  session, which ``logic`` turns into toasts.
* ``ui`` first updates the time range from the store (the span of the data;
  playing moves the window on), then draws the panels and the views, which
  read the store and may change the time range and queue invocations.
  ``Session::send_invocations`` then sends the requests to their sources.
* ``raw_input_hook`` takes mouse wheel events before egui smooths them, so the
  wheel zooms the plot or timeline under the pointer without the rest of a
  notch scrolling what the pointer moves on to (``view::zoom_with_wheel`` and
  ``time::take_wheel``).

An application without the workbench has the same frame, see
:doc:`/library/app`: ``Session::update``, then ``TimeRange::update``, then the
widgets, then ``Session::send_invocations``. blip adds ``Connections::observe``
after ``update``.

What a frame costs follows what is shown, not how much data there is:

* Ingesting a value costs about the same however many arrived before: the
  table of its path is found by its path, the samples are appended to columns.
  A long column grows into a buffer twice as large while it fills its last
  eighth, a few values copied with each one appended, instead of copying all
  at once when full, so also a trace of tens of millions of samples grows
  without a pause.
* A plot reads the lowest and highest value of blocks of 256 samples and picks
  about two points per pixel of its width.
* Views find the sample at a time by a binary search of integer timestamps.
* The traces tree lays out only the rows in sight, the log view only the rows
  in sight and filters only the records of its paths that arrived since the
  last frame, merging them by time with those it shows. When a record arrives
  before others of its path, it filters that path again; it filters all of them
  again only when its filter changes or a path is no longer shown.
* The timeline counts what arrived at most ten times a second, a heatmap
  uploads its image only when it changes.
* A distribution counts the samples of the shown range again in each frame, in
  one pass over the samples found by binary search. The box statistics
  (``distribution/boxes.rs``) are kept by ``view::cached``: they are computed again only when
  the samples version, the shown window, the whiskers or the traces change,
  not on every frame.

Receiving
---------

What happens to a message on its way into the store:

* **Sources and the inbox.** A source is a ``Source`` (see
  :doc:`/library/sources`), in blip one connection, named in the connections;
  the session holds it. When polled it gets an ``Inbox`` and hands over what it has: a frame
  (``Inbox::frame``), e.g. one binary WebSocket message holding one ``Message``
  or several as a CBOR sequence (RFC 8742), e.g. a batch of sonde passed
  through by the outpost, or typed data (``sample``, ``samples``, ``log``,
  ``metadata``, ``actions``, ``hello``, ``action_result``). The inbox holds the
  parser, so a source holds none: it reads a frame message by message in one
  scan, so everything later deals with single messages. Whatever the way in,
  the session's router gets the same thing for it, which it gives to the
  recorder, to the invoker (answers) or to the store as an ``Input``.
* **Paths.** Paths are global, not prefixed with the source. Record fields
  are joined to the path of their record with ``.`` (``imu.acc``), array
  elements written with indices (``imu.acc[0]``); ``fields_start`` marks where
  the field names begin in the path, so the tree can fold a record.
* **Messages.** A ``Sample`` is applied as it comes. A ``Samples`` block is
  read in one pass: each value goes into the rows of the block while its
  timestamps are collected, then the parser checks the form of the borrowed
  times itself (``block_times`` in ``kit/src/store/protocol.rs``), with the same
  rules and error texts as ``proto::SampleTimes::new``, so that reading a block
  allocates nothing; only if all passes is the block applied. So a malformed
  block is dropped whole, with a warning in the log and an error notice, and
  the other messages of the frame are kept. CBOR the protocol does not allow
  inside a value (a byte string, a tag, an indefinite-length array) makes the
  whole message unreadable, not only that value. A message that cannot be read
  is not shown; a recording keeps its bytes. The old names ``Value`` and
  ``ValueMetadata`` are read as ``Sample`` and ``TraceMetadata``. The store
  collects the notices (``Ingest::take_notices``, without repeats),
  ``Session::take_notices`` adds those of the sources and of messages that could
  not be read; ``App::logic`` turns them into toasts, also those of an opened
  recording. A block counts once for the rate of a readback.
* **Hello.** The parser reads the ``Hello`` of a frame like any message; the
  inbox also reports it as an event of the source, so ``Connections::observe``
  can read it from ``Session::source_events`` and show it. The store keeps it as a description of its source and takes the clock from
  it (``Input::Clock``).
* **Kinds.** Integers are stored as ``i64``, as ``u64`` once one is above
  ``i64::MAX``, and the column switches to ``f64`` with the first float.
  Arrays are stored row-major with their shape. A sample of an incompatible
  kind is dropped with a warning in the log, once per trace.
* **Order.** Samples are kept sorted by timestamp; a sample older than the
  newest (e.g. from a second outpost) is moved into place. One older than the
  newest 65 536 values of its path is dropped with a warning, as moving all of
  them for each value of a clock minutes behind would stall blip.
* **Time base.** The store keeps the clock of each source from its latest
  ``Hello``. The base of a sample is the clock of its source, else a guess
  from its timestamp (``TimeBase::guess``: before 10\ :sup:`18` ns is time since
  boot). The first accepted sample or log record, or an epoch ``Hello``, sets
  the store's base, UTC or time since boot, and the first timestamp the
  origin the views count from. A sample or log record of the other base is
  dropped with a single warning, as it would land decades away from the
  others. An epoch ``Hello`` while the base is time since boot removes the
  samples and log records already received (``MemoryStore::drop_samples``, keeping
  metadata, actions and the latest values) and sets the base to UTC;
  later boot samples are dropped. The warning that connections send different
  clocks is given once per store.
* **Metadata** comes from ``TraceMetadata`` and is kept by path in ``Traces``,
  also before the first sample, and through ``drop_samples``. A newer message
  of a path replaces all of the earlier one. Lookups of the unit, the
  description, the labels and the expected range fall back from the path of an
  array element (``imu.acc[0]``) to its array trace (``imu.acc``).
* **Log records** are kept, all of them, by path, in the order of their
  timestamps. A
  record with the timestamp 0 gets the newest time of the data received so
  far (or the wall clock if there is none), so it lines up with the samples
  of its target. Should the first timestamp then be in another time base,
  e.g. an uptime, such records move to it. Levels above 4 count as errors (``proto::level``), a record without a path is
  at the path ``log/default``. Log records have no component: the part that
  wrote one is in its path, by convention ``log/<part>``. A ``component`` key
  of an old recording is ignored when decoding.
* **Recording versus receiving.** Samples are kept only while recording.
  While not, the parser reads a sample message only up to its latest value,
  which the store keeps for the readbacks of actions (``Input::Latest``), and
  skips log records; a source does nothing for this. What a target sends only
  once or seldom -- its hello, its trace metadata and its actions -- is
  kept regardless, as the latest **description** per source and path, in the
  recorder. Clearing the data, or starting a recording, writes these
  descriptions first, so a new recording starts with them and a saved recording
  is complete on its own.
* **Actions.** A source announces all its actions in one ``Actions`` message;
  a new list replaces the old one of that source. Several sources may announce
  an action at the same path; blip keeps each, and an actions view chooses the
  source it sends to.

The store
---------

The widgets read the store through the ``Store`` trait (``store/query.rs``,
see :doc:`/library/stores`), never from the type: each asks for the samples it
shows with a ``Window`` and gets a ``Loaded`` answer. The calls are
synchronous and must not block. The plot asks for the shown time range with
two points per pixel of its width, tooltips and tables ask ``Window::at(t)``,
the trace tree and the checks of what a view can show of a kind ask
``Window::NONE``, the log table asks for all records. ``MemoryStore`` is the
store of blip. It ignores the window and borrows its columns, so nothing is
copied. The ``Session`` holds it and feeds it through ``Ingest``.

``MemoryStore`` owns everything received since the data was last cleared:

* ``Traces``: the trace metadata by path, and a table for each path values are sent to, with a column per leaf
  of its values, its traces, sorted by path so the fields of a record follow
  each other. The columns of a table share its ascending timestamps, so the
  fields of a record are kept together. The first record at a path sets its
  fields: a record with other fields is dropped with a warning, a field whose
  value does not fit its trace is null.
  A column holds ``Samples``: ``Numbers``, ``Strings`` (each different text
  kept once), or ``Arrays`` with a shape and their elements in rows of a
  ``Numbers``. ``Numbers`` keeps one ``f64`` per value for plotting, nulls as a
  NaN of their own, and integers exactly beside them only once one does not
  fit a float. It keeps the lowest and highest value of each block of 256
  rows. ``Traces::series`` gives views the numbers of a trace or of an array
  element as a ``Series``, whose ``extent`` reads the blocks.
* ``Logs``: the log records by path, those of each path in the order of their
  times, so that a log view reads only the records of the paths it shows. A
  record put before others of its path changes the order ``Version`` of that
  path only. The timeline counts the records of all paths.
* ``Actions``: the latest list of actions of each source.
* The **latest value** of each path, also while not recording.

The ``Recorder`` is not in the store: the session holds it beside the store and
gives it each message as it routes it. It encodes the message as it arrives
(``recording.rs``, with the CBOR writer of ``cbor.rs``): integers in their
shortest form, floats in the shortest of half, single and double precision that
reads back the same, a block as a block, the old names ``Value`` and
``ValueMetadata`` as ``Sample`` and ``TraceMetadata``, and the source names
written in between where the source changes. Fields blip does not know in a
message it can read are not kept. A message it cannot read is written as it
arrived. The messages are compressed fast as they arrive, a gzip member per
64 KiB, each a fraction of a millisecond of work, so the view does not stutter;
saving writes the members one after another as they are. It also keeps the
**descriptions**: the latest hello, metadata and actions of each source, with
which a recording that starts later begins.

Two ``Version`` counters let views and the timeline cache work: the
``paths_version`` changes when a trace, the first log record of a path or a
list of actions arrives (views
expand their wildcards only then), the ``samples_version`` when a value or log
record arrives.

Opening a recording file creates a fresh ``MemoryStore`` and reads the file into
it through ``Ingest`` with a ``FileReader``, which reads the file's messages with the
same parser as those of a frame. ``FileReader::finish`` gives the
``Recorder`` of the file, which holds it as it was opened, and
``Session::start_fresh(store, recorder)`` puts both in place of the old ones.
Saving an opened recording again writes the file as it was opened, byte for byte,
with its version, until a new recording starts.

The time range
--------------

``TimeRange`` holds the window length, the end of the window (``None`` while
live), whether it is playing, the cursor (``None`` while at the end of the
window) and the zoom of the timeline. Each frame ``update`` reads the span of
the data from the store and moves the window on while playing; when it
reaches the newest data it goes live. Views call ``pan_zoom``, ``set_cursor``
and ``go_to``, which pause where the user moved away from live. Only the
window length is persisted.

Its times are seconds as floats, counted from the **origin**, the first
timestamp the store received, so they keep nanoseconds also at Unix times.
The **time base** of the time axes, the log table and the log tooltips is the
one of the store (``MemoryStore::time_base``, time since boot while there is none),
set by the clock of the ``Hello`` or guessed, not guessed again from the
latest timestamp.
Views turn them into integer nanoseconds with ``TimeRange::timestamp`` to look
up samples, and the time axes write the times from those nanoseconds.

The timeline counts what arrived in bins of a round duration with
``Store::activity`` -- a record once, as the values of its table -- and counts
again when the bins change, or the samples changed and a tenth of a second
passed.

``TimeRange::ui`` draws the controls with the timeline; ``controls_ui`` and
``timeline_ui`` draw them apart, for an application that lays them out itself.
``toggle_play`` and ``release_cursor`` do what the play button and Escape do,
and ``set_connected`` tells whether a connection is open, for the hint of the
empty timeline. The window length is one field with a menu of round lengths
(10 ms to 1 h); ``parse_duration`` reads the units in any case, and an entry
it cannot read stays in the field, marked, until replaced. The timeline's
scrubbing of the ruler moves the window with the cursor.

Layout and views
----------------

The ``Layout`` wraps an ``egui_tiles::Tree<View>``: containers are the groups
(tabs, vertical, horizontal; group names are kept beside the tree), panes are
the views. blip draws the tree with its own ``Behavior`` (cards, tab bars,
titles) and does its own drag and drop with the dragged ``TileId`` as payload;
``drag.rs`` finds where the pointer is and what a drop does there. Groups nest
to any depth.

* A drop is a **side** of a view (where the group stacks along that side, the
  dragged tile moves next to the view; otherwise both form a new group, or a
  group of just the two turns), the **middle** of a view (the view inset by a
  quarter, at least 40 by 40 px: a new group of tabs of both, or a vertical
  or horizontal group of just the two turned into tabs), or a **tab row**
  (a tab button or the empty part of the row, at every level including the
  top: the tile becomes a tab there).
* Adding a view at a side from the menu of a view goes through the same drop.
  The menu of a view and of a nested tab group has **Add right** and **Add
  below** with the view types and then **Empty group**, a group of tabs with
  one empty tab. Every tab bar has a **+** that adds an empty tab.
* ``picker.rs`` draws the view types of an empty tab. ``view_picker`` picks a
  form from the room the tab has: buttons with a name, a row of icon buttons,
  or one menu button.
* A group of tabs below the top level is drawn as a **tabbed card**: the tabs
  sit in the title row of one card, also for an empty tab. The tabs at the top
  lie on the canvas.
* ``remove_if_empty`` prunes after a move or a delete: a vertical or horizontal
  group, and a group of tabs below the top, left without children is removed,
  and its parent if that is left empty too. A tab that is emptied by a move is
  removed; one emptied by a delete stays (``keep_tab``). The top level stays.

A ``View`` has a name, a ``ViewKind`` with the state of its type (a variant for
each built-in type, and ``Custom`` for a type an application registered), the number
``Formats`` of its traces and the ``wildcards`` (branch keys checked in the
tree). The state of each type implements ``Viewable``, how it is shown, and
``ViewInfo``, what it is:

.. code-block:: rust

   pub trait Viewable {
       fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>);
       fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {}
       /// The paths it shows of the traces tree, and whether as records.
       fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> { None }
   }

   pub trait ViewInfo {
       const NAME: &'static str;
       const ICON: &'static str;
       // HAS_SELECTION, WRITES_VALUES, SHOWS_ONE_TRACE and SHOWS_ACTIONS
       fn selectable(trace: Trace<'_>) -> Selectable;
       fn shows_apart(trace: Trace<'_>) -> bool { false }
   }

``ViewContext`` gives a view the store (``&dyn Store``), the time range (mutable, to pan and
set the cursor), the invoker (to queue invocations), its formats and the
``Plots`` of the layout; ``PropertiesContext`` gives the properties the
``Plots`` too.
``selectable`` decides per trace what a view type can show -- the trace, its
elements, the whole array, or nothing with a reason -- which the traces tree
uses to enable checkboxes and to filter.

``view::showing::<V>(paths)`` makes a view with its traces in code, for an
application without the layout. ``ViewContext::new(&store, &mut time, &mut
invoker)`` makes a context with the default formats and no plots.

To add a view type to blip: add its struct with ``Default``, ``Viewable`` and
``ViewInfo`` in a file in ``widgets/view``, and a line to the list in ``views!`` in
``widgets/view/mod.rs``, which makes ``ViewKind``, ``ViewType`` and their
dispatch. The name of the line is saved in presets. Its settings are saved
with the layout through serde; give new fields defaults (``#[serde(default)]``)
so older presets still load. ``ViewType::BUILT_IN`` lists the types of the
list, ``ViewType::all()`` those and the registered ones; the menus use the
latter.

An application adds its own view types with ``custom::register::<T>()``
(``widgets/view/custom.rs``, written for users in :doc:`/library/widgets`). The
registry is a static list of ``CustomType``\ s, which keep what ``ViewInfo``
says of the type and functions to make a view and to load one from JSON.
``ViewKind::Custom`` holds a ``CustomView``: the state behind a trait object,
the name of its type and, for a type that is not registered, the saved state.
It is saved as ``{"Custom": {"type": name, "state": ...}}`` and a view of an
unknown type is the *Unknown* placeholder: it shows nothing, keeps the saved
state and writes it back unchanged. The views of the built-in types are saved
as before, so presets of earlier versions load unchanged.

A ``Table`` can carry columns of the application (``Table::column``) with a
``TableCell`` for each row. They are code, not saved with the table.

Tooltips
~~~~~~~~

``view/tooltip.rs`` draws the hover tooltips of the plot-like views. A
``Tooltip`` has a ``Header`` and groups of ``Row``\ s with a rule between the
groups. A row is a ``Label``, either a trace's dot and name or weak text, and
right-aligned ``Value``\ s in the tones strong, text, weak or faint. A view
builds the tooltip from what is under the pointer, so all of them keep the
same frame, spacing and colours.

A labelled number is a ``Value`` with the label and, in the faint tone, the
number (``Value::labelled``). A ``Footer`` below a rule holds the short name of
the trace under the pointer and its description, wrapped to the larger of the
grid's width and 200 points and cut with an ellipsis after three rows
(``Tooltip::describe``). Which trace that is depends on the view: the band row
under the pointer in States (``row_at``), the curve whose value at the hovered
time is nearest the pointer within 12 points in Plot (``nearest_curve``), the
trace of the header in the others.

Trace metadata in the views
~~~~~~~~~~~~~~~~~~~~~~~~~~~

* **Labels** is a ``DisplayFormat``, offered first for number and array traces
  that declare labels (``DisplayFormat::of_kind``). ``Formats`` keeps a choice
  only when it differs from the default of the trace (Labels if it has labels,
  else Default), so layouts saved before give labelled traces Labels.
  ``Number::format_labelled`` writes the label, or the number if it has none.
  A band's colour follows the number, except for a label that is a log level
  name. Stats and distributions keep numbers.
* **The Traces section** of the properties (``traces_ui``) shows per trace its
  format, unit, description, expected range and labels
  (``trace_facts_rows``); the Records layout of a table adds the same rows
  under each column. ``trace_hover`` writes the hover text of the tree, the
  table and the Records headers.
* **Ends.** The ends of the ranges of the axes of Plot (``y_min``, ``y_max``),
  XY, the heatmap and the distribution are set in one place, ``range_rows``,
  which draws *Fit to* (while a trace declares a range) and the **Min** and
  **Max** fields. The XY view, the heatmap and the distribution keep the ends
  as ``Ends``, each end ``None`` to fit; it reads the fixed ``[a, b]`` and
  ``null`` of older presets. ``Ends::over`` gives the range in use over the
  fitted one and ``problems`` the marks of wrong entries (not a number, min
  not below max).
* **Fit.** ``Fit`` (``Expected`` or ``Values``) is saved in ``Plot`` and
  ``Xy``, default ``Expected``, so older layouts get it. The row *Fit to* shows
  only while a shown trace declares a range. ``Plot::range_y`` takes the union
  of the declared ranges and the values of the traces without one, with the
  usual margin, values beyond the declared range growing it; ``Xy::axis_ranges``
  does the same for both axes. The heatmap's Auto colour scale is the declared
  range of its array.

Undoing a delete
~~~~~~~~~~~~~~~~

Deleting a view, tab or group (``Layout::delete_selected`` or the menus) and
``Layout::clear`` keep the layout from before, with the selection to restore.
``Layout::take_deleted`` gives the name for the toast, *Deleted Plot 1* or *all
views*, which the app shows with an *Undo* button tagged with
``Layout::undo_number``. ``undo_delete`` brings the layout back and keeps the
edits made to the views that were not deleted, e.g. their traces. Any other
change of the layout, e.g. a move, forgets the kept layout, as undoing it would
lose that change; ``can_undo_delete`` tells whether it is still there, and the
app closes the toast of a delete that no longer can be undone.

Links between views
~~~~~~~~~~~~~~~~~~~

A distribution can be linked to a plot, the only link between views: it shows
the plot's traces and its y range. A view refers to another by a ``ViewId``,
the id of its tile in the tree. Each frame, ``Layout::take_linked_traces``
copies the traces of each linked plot into the distribution, with the plot
view's wildcards and number formats, before anything shows them; ``App::ui``
calls it before the side panels and ``Layout::ui`` again before the views.
The distribution keeps them when unlinked. ``Layout::selected_link`` names the
plot of a selected linked distribution, for the greyed tree of the traces. Each
frame, ``Layout::plots`` lists the plot views in the order of the
layout as ``Plots``: the ``ViewId``, the name and the y range each shows
(``Plot::shown_y``: the range panned or zoomed to, else the fixed min and max,
with the ends not set fitted to all its data). The layout takes the list before
it draws the views and passes it to all of them, so a plot zoomed or panned in
a frame shows its new range to the distributions in the next; the layout repaints
when a range changed during the frame.

``View::relink`` maps the links of a view to other ids and drops those it maps
to none. Deleting views drops the links to them. In the saved form a link is
the number of the view in the saved order, e.g. ``"link": 0`` for the first
view of the layout: ``Group::save`` numbers the views as it saves them and
``Group::load`` turns the numbers back into the tiles it inserts. A new kind
of link is mapped in ``View::relink`` too.

Presets
~~~~~~~

The ``Layout`` and the ``Connections`` shown are those of the active preset.
``Layout::to_saved`` gives the saved form of the layout, the ``Group`` of the
tabs at the top level with the groups and views below (``layout/file.rs``),
and ``Connections::saved`` the names and addresses. ``App::store_preset``
writes both into the active ``Preset`` before the presets are kept, exported
or switched; ``App::show_preset`` loads the active one and keeps the open
connections it has too, by name and address.
Changing the active preset is not offered while recording, since it changes
the connections.

``PresetsFile`` is the one saved form, for the kept presets and the files: a
version and the presets, as JSON, described for users in
:doc:`/reference/files`. ``presets::read`` reads the version first and
brings older versions up to date, one version after another, before reading
the presets; version 1 needs no migration, as version 2 only adds values and a
field with a default. A file of a newer version is refused, and so are layout files of
the versions before presets and presets without a name or with the same name.
The connections read are kept apart as editing keeps them
(``Connection::sanitized``). Format changes must add a migration there, not
drop old presets, and a line to the versions in :doc:`/reference/files`.

Actions and the invoker
-----------------------

The ``Invoker`` sends invocations and tracks their answers. An invocation
carries an id unique within the connection; the invoker tracks each pending
invocation per (source, path) until its ``ActionResult`` arrives or five
seconds pass. A readback is metadata only: blip shows it and lets toggles and
choices follow it, but never infers success from it.

* ``invoke`` queues an ``Invoke`` request with a new id for an action of a
  source; ``invoke_dragging`` and ``finish_dragging`` throttle a dragged
  slider to one request per 100 ms and send the final value.
* ``App::send_invocations`` calls ``Session::send_invocations``, which takes
  the queue and sends each request to its source, unless the source no longer
  announces the action. Sources may share a name, e.g. two connections without
  one; a request to a name goes to the open source of that name that announced
  its actions last, the set the user sees, else to the first open one.
* ``receive`` matches an ``ActionResult`` to its pending invocation;
  ``update`` marks invocations unanswered after five seconds and knows which
  sources are connected.
* For readbacks, the invoker remembers how many readback samples there were
  when an answer arrived, so toggles and choices follow the readback only once
  a newer sample arrives.

Persistence
-----------

eframe's storage keeps the serialised ``App`` (window length, panel state;
the data, the layout and the connections are skipped) under its app key, the
JSON of the ``PresetsFile`` under ``presets`` and the name of the active
preset under ``active preset``. The workbench does not depend on eframe: the
``Presets`` read and write through their own ``Storage`` trait
(``get_string`` and ``set_string``), and ``blip/src/app.rs`` adapts eframe's
storage to it. An application that keeps its presets elsewhere implements it. Kept presets of a newer version are not
overwritten, damaged ones are kept under ``unread presets``. eframe saves every 30 seconds and on exit; on the web,
``main.rs`` also saves when the page is hidden or left, as eframe's own save
on closing does not run there. egui keeps its memory, e.g. the zoom and the
theme.

Web and native
--------------

* **Web**: ``trunk build`` compiles to ``wasm32-unknown-unknown`` and bundles
  ``index.html``, the icons, the manifest and a service worker
  (``assets/sw.js``), so the app can be installed and works offline. Files
  open through a file input and save as downloads.
* **Native**: eframe with the ``glow`` renderer, ``rfd`` file dialogs,
  ``env_logger`` (set ``RUST_LOG=info`` for the connection log).

Checks and tests
----------------

``check.sh`` runs what CI runs: ``cargo check`` natively and for WASM, the
builds of the features of ``blip-kit`` (the store alone, natively and for the
web, then with the widgets), a check that the store alone pulls in no GUI
crate, the build of the example ``dashboard`` for the web, ``rustfmt``,
``clippy``, the tests, the API documentation of the crates with warnings as
errors, ``cargo package --no-verify`` of both library crates, as they would be
published, and a ``trunk build``. ``check.sh --fast`` stops after the tests,
for the checks while a change is in work. cargo calls ``rustc`` through
``.cargo/rustc-wrapper.sh``, which uses ``sccache`` when it is installed, so the
dependencies are compiled once for all checkouts and worktrees. UI behaviour is
tested with ``egui_kittest`` harnesses next to the code, e.g. clicking the
record button or dragging in the trace tree; connection tests run against a
local ``tungstenite`` WebSocket server.

The screenshots of this documentation are tests of the ``blip`` crate too
(``blip/src/app/screenshots.rs``,
behind the ``screenshots`` feature and ignored by default): they render the
app with wgpu from the recording ``tests/data/demo.blip`` and the preset
``tests/data/demo-presets.json``, the window connected to a WebSocket server in
the test as to an outpost. ``make screenshots`` in ``doc`` runs them;
``make screenshots SCREENSHOTS_DIR=…`` writes them to another directory, so
they can be compared with the committed ones.

The library has its own tests. ``kit/examples/dashboard.rs`` has a test that
runs the application in a harness, receives from its simulator and clicks the
button of a row. ``kit/tests/store.rs`` reads a plot from a second ``Store``
implementation and checks what it asked for.
``kit/tests/baseline.rs`` measures how many samples a second the store takes in
and how long a frame of a plot of a long trace takes; its tests are ignored by
default, see :doc:`/library/stores`.
