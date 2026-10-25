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
     - What was received: ``traces`` (with ``numbers``, the columns that keep
       integers exact), ``logs``, ``actions`` (the announced actions and the
       ``Invoker``) and ``recording`` (the received messages, and reading and
       writing recording files).
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

The ``proto`` crate holds the wire types; blip decodes them with
``serde_cbor_2``.

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

The model
---------

``Model`` owns everything received since the data was last cleared:

* ``Traces``: a map from path to ``Trace``, sorted so the fields of a record
  follow each other. A trace has ascending ``timestamps`` and ``Samples``:
  ``Numbers`` (a column of ``i64``, ``u64`` or ``f64`` values or bools, with
  nulls), ``Strings``, or ``Arrays`` with a shape and their elements row-major
  in one column. ``Traces::series`` gives views the numbers of a trace or of an
  array element as a ``Series`` with plotting helpers.
* ``Logs``: the log records in arrival order.
* ``Actions``: the latest list of actions of each source.
* ``Recording``: the received messages as one byte buffer, with the source
  names written in between where the source changes -- the body of a
  recording file.
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

The timeline counts what arrived in bins of a round duration with
``Model::activity`` -- records counted once, by their field with the most
values -- and recounts only when the bins or the ``samples_version`` change.

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
tree). Each kind implements ``Viewable``:

.. code-block:: rust

   pub trait Viewable {
       fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>);
       fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {}
   }

``ViewContext`` gives a view the model, the time range (mutable, to pan and
set the cursor), the invoker (to queue invocations) and its formats.
``ViewType::selectable`` decides per trace what a view type can show -- the
trace, its elements, the whole array, or nothing with a reason -- which the
traces tree uses to enable checkboxes and to filter.

To add a view type: add the struct and its ``Viewable`` implementation in
``app/view``, a variant to ``ViewKind`` and ``ViewType`` (name, icon,
``selectable``), and its arms in ``View::new``, ``View::view_type`` and
``View::selection_mut``. Its settings
are saved with the layout through serde; give new fields defaults
(``#[serde(default)]``) so older layouts still load.

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
