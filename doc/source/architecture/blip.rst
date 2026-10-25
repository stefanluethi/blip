blip
====

blip is an `egui <https://github.com/emilk/egui>`_/eframe application in the
``blip`` crate. The same code compiles to WebAssembly, served by Trunk, and to
a native binary. It is an immediate-mode GUI: each frame, blip applies what
arrived and draws the whole window again from its state.

Structure
---------

.. figure:: /diagrams/blip-modules.svg
   :target: ../_images/blip-modules.svg
   :class: diagram
   :alt: The App holds the State (time range, model, invoker), the
         connections, the files, the toasts and the layout with its views; the
         connections exchange binary frames with outposts; all use the proto
         crate.

   Modules of ``blip/src``.

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Module
     - Responsibility
   * - ``main.rs``
     - Starts eframe natively or on the canvas of ``index.html`` on the web;
       on the web also saves the state when the page is hidden.
   * - ``app.rs``
     - ``App``: the panels of the window, the menu, opening and saving files,
       persistence. ``State``: the time range, whether recording, the model
       and the invoker, and the rules between them (a recording starts only
       while connected and clears the data; the first connection to open
       starts afresh).
   * - ``app/connections.rs``
     - The connections panel and one ``ewebsock`` socket per open address.
       Splits frames into messages and passes them on with the name of their
       connection; sends requests.
   * - ``app/model``
     - What was received: ``traces`` (with ``numbers``, the columns of numbers),
       ``logs``, ``actions`` (the announced actions and the ``Invoker``) and
       ``recording`` (the received messages, and reading and writing recording
       files); ``cbor`` and ``decode`` read the messages.
   * - ``app/time.rs``, ``app/time/timeline.rs``
     - The ``TimeRange`` shared by all views, its controls and the timeline
       with the activity bars.
   * - ``app/layout``
     - The ``Layout``: an ``egui_tiles`` tree of views, the layout panel
       (``panel.rs``), the properties panel (``properties.rs``), dragging in
       the center (``drag.rs``) and the saved form (``file.rs``).
   * - ``app/view``
     - The views and what they share: the ``Viewable`` trait, the
       ``trace_selection`` tree, the ``time_axis`` of the views over time and
       the properties widgets.
   * - ``app/files.rs``
     - Open and save dialogs (``rfd`` natively, a file input and a download
       on the web) and dropped files.
   * - ``app/toasts.rs``, ``app/theme.rs``
     - The notices in the bottom right, and the colours, fonts and shared
       widgets of the look.

The ``proto`` crate holds the wire types. Values and log records, which arrive
all the time, are read by blip's own CBOR reader (``model/cbor.rs`` and
``model/decode.rs``) in one pass: it borrows the strings of the frame and
flattens a value into its leaves, the fields of records and their arrays of
numbers, into a buffer kept for the next value. The other messages are decoded
with ``serde_cbor_2``.

The frame
---------

eframe calls ``App::logic`` and then ``App::ui`` for each frame. blip repaints
when there is input, and the sockets wake it up when a message arrives, so it
draws as often as data arrives, not more.

.. figure:: /diagrams/blip-frame.svg
   :target: ../_images/blip-frame.svg
   :class: diagram
   :alt: logic(): receive from the connections, split frames, apply to the
         state and model, pass answers to the invoker, update the invoker,
         open files. ui(): menu, update the time range, bottom, left and right
         panels, the views, then send the invocations.

   What happens in a frame.

* ``logic`` drains all sockets. Each message goes through ``State::apply`` to
  ``Model::receive`` (while recording) or ``Model::receive_unrecorded``
  (otherwise: only units, actions and answers). Answers to invocations go to
  the invoker.
* ``ui`` first updates the time range from the model (the span of the data;
  playing moves the window on), then draws the panels and the views, which
  read the model and may change the time range and queue invocations.
  ``send_invocations`` then sends what the views queued.
* ``raw_input_hook`` takes mouse wheel events before egui smooths them, so the
  wheel zooms the plot or timeline under the pointer without the rest of a
  notch scrolling what the pointer moves on to.

What a frame costs follows what is shown, not how much data there is:

* Ingesting a value costs about the same however many arrived before: the
  table of its path is found by its path, the samples are appended to columns.
* A plot reads the lowest and highest value of blocks of 256 samples and picks
  about two points per pixel of its width.
* Views find the sample at a time by a binary search of integer timestamps.
* The traces tree lays out only the rows in sight, the log view only the rows
  in sight and filters only the records that arrived since the last frame.
* The timeline counts what arrived at most ten times a second, a heatmap
  uploads its image only when it changes.

The model
---------

``Model`` owns everything received since the data was last cleared:

* ``Traces``: a table for each path values are sent to, with a column per leaf
  of its values, its traces, sorted by path so the fields of a record follow
  each other. The columns of a table share its ascending timestamps, so the
  fields of a record are kept together; a field missing from a value is null.
  A column holds ``Samples``: ``Numbers``, ``Strings`` (each different text
  kept once), or ``Arrays`` with a shape and their elements in rows of a
  ``Numbers``. ``Numbers`` keeps one ``f64`` per value for plotting, nulls as a
  NaN of their own, and integers exactly beside them only once one does not
  fit a float. It keeps the lowest and highest value of each block of 256
  rows. ``Traces::series`` gives views the numbers of a trace or of an array
  element as a ``Series``, whose ``extent`` reads the blocks.
* ``Logs``: all log records in the order of their times.
* ``Actions``: the latest list of actions of each source.
* ``Recording``: the received messages, with the source names written in
  between where the source changes, compressed as they arrive into a gzip
  member per MiB -- the members one after another are the recording file.
* The **descriptions**: the latest units and actions frame of each source,
  re-applied by ``Model::clear``.

Two ``Version`` counters let views and the timeline cache work: the
``paths_version`` changes when a trace or a list of actions arrives (views
expand their wildcards only then), the ``samples_version`` when a value or log
record arrives.

Opening a recording file creates a fresh ``Model`` and feeds it the file's
messages with ``Model::receive``, exactly as if they had been received.

The time range
--------------

``TimeRange`` holds the window length, the end of the window (``None`` while
live), whether it is playing, the cursor (``None`` while at the end of the
window) and the zoom of the timeline. Each frame ``update`` reads the span of
the data from the model and moves the window on while playing; when it
reaches the newest data it goes live. Views call ``pan_zoom``, ``set_cursor``
and ``go_to``, which pause where the user moved away from live. Only the
window length is persisted.

Its times are seconds as floats, counted from the **origin**, the first
timestamp the model received, so they keep nanoseconds also at Unix times.
Views turn them into integer nanoseconds with ``TimeRange::timestamp`` to look
up samples, and the time axes write the times from those nanoseconds.

The timeline counts what arrived in bins of a round duration with
``Model::activity`` -- a record once, as the values of its table -- and counts
again when the bins change, or the samples changed and a tenth of a second
passed.

Layout and views
----------------

The ``Layout`` wraps an ``egui_tiles::Tree<View>``: containers are the groups
(tabs, vertical, horizontal; group names are kept beside the tree), panes are
the views. blip draws the tree with its own ``Behavior`` (cards, tab bars,
titles) and does its own drag and drop: the layout panel and the center share
the dragged ``TileId`` as payload, and ``drag.rs`` finds the side of the view
under the pointer.

A ``View`` has a name, a ``ViewKind`` with the state of its type, the number
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

``ViewContext`` gives a view the model, the time range (mutable, to pan and
set the cursor), the invoker (to queue invocations) and its formats.
``selectable`` decides per trace what a view type can show -- the trace, its
elements, the whole array, or nothing with a reason -- which the traces tree
uses to enable checkboxes and to filter.

To add a view type: add its struct with ``Default``, ``Viewable`` and
``ViewInfo`` in a file in ``app/view``, and a line to the list in ``views!`` in
``app/view/mod.rs``, which makes ``ViewKind``, ``ViewType`` and their
dispatch. The name of the line is saved in layouts. Its settings are saved
with the layout through serde; give new fields defaults (``#[serde(default)]``)
so older layouts still load.

The saved layout
~~~~~~~~~~~~~~~~

``LayoutFile`` is the saved form: a version and the tree of groups and views,
as JSON. It is written to the app storage under its own key and by **Save
layout**. ``LayoutFile::migrate`` brings older versions up to date (version 2:
the x trace of an XY view is one of its traces; version 3: ``streams`` renamed
to ``traces``); a layout of a newer version is refused. Format changes must
add a migration, not drop old layouts.

Actions and the invoker
-----------------------

The ``Invoker`` sends invocations and tracks their answers:

* ``invoke`` queues an ``Invoke`` request with a new id for an action of a
  source; ``invoke_dragging`` and ``finish_dragging`` throttle a dragged
  slider to one request per 100 ms and send the final value.
* ``App::send_invocations`` takes the queue and sends each request to its
  source, unless the source no longer announces the action.
* ``receive`` matches an ``ActionResult`` to its pending invocation;
  ``update`` marks invocations unanswered after five seconds and knows which
  sources are connected.
* For readbacks, the invoker remembers how many readback samples there were
  when an answer arrived, so toggles and choices follow the readback only once
  a newer sample arrives.

Persistence
-----------

eframe's storage keeps the serialised ``App`` (window length, connections,
panel state; the data is skipped) under its app key and the ``LayoutFile``
under ``layout``. eframe saves every 30 seconds and on exit; on the web,
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

``check.sh`` runs what CI runs: ``cargo check`` natively and for WASM,
``rustfmt``, ``clippy``, the tests and a ``trunk build``. UI behaviour is
tested with ``egui_kittest`` harnesses next to the code, e.g. clicking the
record button or dragging in the trace tree; connection tests run against a
local ``tungstenite`` WebSocket server.

The screenshots of this documentation are tests too (``app/screenshots.rs``,
behind the ``screenshots`` feature and ignored by default): they render the
app with wgpu from the recording ``tests/data/demo.blip`` and the layout
``tests/data/demo-layout.json``, the window connected to a WebSocket server in
the test as to an outpost. ``make screenshots`` in ``doc`` runs them.
