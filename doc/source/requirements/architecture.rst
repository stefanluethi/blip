Architecture Elements
=====================

The components of the software (``ARCH_*``) and the requirements each
satisfies. How each is built is described on the internals page of its part.

.. arch:: blip frontend
   :id: ARCH_BLIP
   :status: implemented
   :component: blip
   :satisfies: REQ_VIS_PLOT, REQ_VIS_LOG, REQ_VIS_LOG_DEFAULT, REQ_VIS_LOG_PICK, REQ_VIS_LOG_PATH_FILTER, REQ_VIS_STATES, REQ_VIS_TABLE, REQ_VIS_XY, REQ_VIS_HEATMAP, REQ_VIS_HIST, REQ_VIS_HIST_BINS, REQ_VIS_HIST_COUNTS, REQ_VIS_HIST_MARKERS, REQ_VIS_HIST_ORIENT, REQ_VIS_HIST_LINK, REQ_VIS_HIST_HOVER, REQ_VIS_HIST_EMPTY, REQ_VIS_HIST_SAVED, REQ_VIS_DIST_SHOW, REQ_VIS_DIST_BOX, REQ_VIS_DIST_WHISKERS, REQ_VIS_DIST_STATS, REQ_VIS_DIST_BOX_HOVER, REQ_VIS_DIST_BOX_EMPTY, REQ_VIS_DIST_PROPS, REQ_VIS_CURSOR, REQ_VIS_TOOLTIP, REQ_CONN_WS, REQ_PLAT_WEB, REQ_PLAT_NATIVE, REQ_PERSIST_STATE, REQ_PRESETS, REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK, REQ_CTRL_VIEW, REQ_CTRL_SAFE, REQ_PROTO_TIME, REQ_PROTO_LOG_NOW, REQ_PROTO_LEVELS, REQ_PROTO_NAMES, REQ_PROTO_METADATA, REQ_PROTO_SAMPLES_APPLY, REQ_CONN_HELLO, REQ_CONN_HELLO_SHOW, REQ_CONN_NEWER, REQ_CONN_TIMELINE, REQ_FILE_VERSION, REQ_META_LABELS, REQ_META_PROPS, REQ_META_TOOLTIP, REQ_META_HOVER, REQ_META_FIT_PLOT, REQ_META_FIT_XY, REQ_META_HEATMAP, REQ_META_SAVED, REQ_LIB_CRATES, REQ_LIB_REFERENCE, REQ_LIB_NO_CHANGE, REQ_LIB_COMPAT, REQ_LIB_PLATFORMS, REQ_VIS_LOG_FOLLOW, REQ_VIS_LOG_CURSOR, REQ_VIS_LOG_ROWS, REQ_VIS_EMPTY, REQ_VIS_RANGE, REQ_VIS_PLOT_AXES, REQ_VIS_LEGEND, REQ_VIS_XY_PROPS, REQ_VIS_HEATMAP_PROPS, REQ_SEL_MARKS, REQ_SEL_FILTER, REQ_SEL_KEYS, REQ_LAY_UNDO, REQ_LAY_TABS, REQ_LAY_PROPS, REQ_VIS_LIVE, REQ_VIS_PLAY, REQ_VIS_WINDOW, REQ_VIS_TIMELINE, REQ_VIS_RECORD_HINT, REQ_APP_SHORTCUTS, REQ_APP_NOTICES, REQ_APP_LABELS, REQ_CONN_STATUS, REQ_CONN_EDIT, REQ_PRESETS_DIALOGS, REQ_FILE_UNSAVED, REQ_FILE_OPEN, REQ_FILE_SAVED, REQ_CTRL_TYPED, REQ_CTRL_PENDING

   egui/eframe application compiled to WebAssembly (served by Trunk) or to a
   native binary: ``main.rs`` and the ``App`` in ``blip/src``, built on the
   library ``blip-kit`` in ``kit/src`` (``store``, ``widgets`` and
   ``workbench``). Main modules, see :doc:`/blip/internals`:

   * ``app.rs`` -- the panels of the window, the menu, the keyboard shortcuts,
     the question before an unsaved recording is dropped, files and
     persistence, and the ``State`` of the time range and the session with the
     store, the invoker and whether it records. ``main.rs`` also holds the
     size of the native window and the warning of the browser on reload.
   * ``workbench/presets.rs`` -- the presets, each a layout with its connections,
     their menu and dialogs and their versioned saved form, kept and in
     files.
   * ``workbench/connections.rs`` -- the WebSocket connections and their panel,
     with the reasons of refused edits, the confirmation of a delete and the
     ``Status`` of all of them; splits frames into messages and keeps the
     ``Hello`` of each connection to show it, with the warning of a newer
     protocol.
   * ``store`` -- the data model: ``traces`` with the samples and the
     metadata of each (blocks of samples are decoded whole), the clock of each
     source from its ``Hello`` and the time base, ``logs``, the
     announced ``actions`` with the ``Invoker``, which sends invocations and
     tracks their answers, the ``recording`` of the received messages, and
     ``sorted``, the searches of ascending timestamps.
   * ``widgets/time.rs`` -- the time range shared by all views, its controls
     (Live, Play, the field of the window length) and the timeline.
   * ``workbench/layout`` -- the layout of the views as a tree of groups, shown
     with ``egui_tiles``, with the undo of a delete, the properties panel and
     its saved form.
   * ``workbench/toasts.rs`` -- the notices, of three levels, with a button of
     their own where an action is offered, such as Undo.
   * ``widgets/view`` -- views: time-series ``plot``, ``states`` over time,
     ``table`` of values, ``xy`` plot, ``distribution`` of values, ``heatmap``
     of arrays, ``log_table`` and the ``actions`` form; ``time_axis`` shared
     by the views over time, ``plotting`` shared by the plot views with the
     legends, ``tooltip`` shared by their hover tooltips, and the
     ``trace_selection``, the tree of the traces, actions and logs to choose those of
     the selected view. The fields that set the ends of an axis, with their
     marks of errors, are shared by ``plot``, ``xy``, ``heatmap`` and
     ``distribution`` in ``mod.rs``, which also holds the ``Ends`` that the
     last three keep.

.. arch:: blip-proto crate
   :id: ARCH_PROTO
   :status: implemented
   :component: proto
   :satisfies: REQ_PROTO_CBOR, REQ_PROTO_LOG, REQ_CTRL_ANNOUNCE, REQ_PROTO_TIME, REQ_PROTO_LEVELS, REQ_PROTO_NAMES, REQ_PROTO_METADATA, REQ_PROTO_HELLO, REQ_PROTO_SAMPLES, REQ_PROTO_TYPES, REQ_LIB_PROTO_CRATE, REQ_LIB_PUBLISH

   Rust crate defining the serde-serialisable wire types. It is the single
   source of truth for the message schema: ``Hello``, ``Sample``, ``Samples``,
   ``TraceMetadata``, ``Log`` with its ``level`` constants, the actions, the
   protocol ``VERSION`` and the old names ``Value`` and ``ValueMetadata`` as
   aliases when reading. The outpost mirrors it in
   ``outpost/src/blip_outpost/proto.py``.

.. arch:: blip-kit core
   :id: ARCH_KIT_CORE
   :status: implemented
   :component: blip-kit
   :satisfies: REQ_LIB_CRATES, REQ_LIB_FEATURES, REQ_LIB_NO_GUI, REQ_LIB_PLATFORMS, REQ_LIB_COMPAT, REQ_LIB_SOURCE, REQ_LIB_SOURCE_OWN, REQ_LIB_SESSION, REQ_LIB_STORE, REQ_LIB_STORE_ONLY, REQ_LIB_STORE_MEMORY, REQ_LIB_STORE_INFLUX, REQ_LIB_ACTIONS, REQ_LIB_PUBLISH, REQ_LIB_UNSAVED

   The part of the ``blip-kit`` library that is always built and has no GUI
   dependency: the ``Store`` and ``Source`` traits, the in-memory store, the
   recording files, the invoker of actions and the session that does the
   receive and send logic of each frame and knows whether the recording is
   unsaved. It uses the wire types of
   ``blip-proto``.

.. arch:: blip-kit widgets
   :id: ARCH_KIT_WIDGETS
   :status: implemented
   :component: blip-kit
   :satisfies: REQ_LIB_FEATURES, REQ_LIB_PLATFORMS, REQ_LIB_WIDGETS, REQ_LIB_WIDGETS_OWN, REQ_LIB_TABLE_COLUMNS, REQ_LIB_ACTIONS, REQ_LIB_EXAMPLE, REQ_LIB_TIME_API

   Feature ``widgets`` of ``blip-kit``, on by default and needing egui: the
   theme, the time range controls, the timeline, all views and the action
   widgets, each usable on its own in any egui UI and configured in code, the
   tree to choose the traces of a view, and the registry of the view types an
   application adds.

.. arch:: blip-kit workbench
   :id: ARCH_KIT_WORKBENCH
   :status: implemented
   :component: blip-kit
   :satisfies: REQ_LIB_FEATURES, REQ_LIB_PLATFORMS, REQ_LIB_REGISTRY, REQ_LIB_NOTICES, REQ_LIB_LAYOUT_UNDO, REQ_LIB_CONN_STATUS

   Feature ``workbench`` of ``blip-kit``, needing ``widgets``: the
   user-configurable layout with the properties of its views, the presets, the
   list of connections, the toasts and the file dialogs. The shipped ``blip`` application is built on it.

.. arch:: blip-kit WebSocket source
   :id: ARCH_KIT_WEBSOCKET
   :status: implemented
   :component: blip-kit
   :satisfies: REQ_LIB_FEATURES, REQ_LIB_PLATFORMS, REQ_LIB_SOURCE_WS

   Feature ``websocket`` of ``blip-kit``, on by default: the ``Source`` for
   the WebSocket connection to an outpost, for web and native.

.. arch:: Example application
   :id: ARCH_LIB_EXAMPLE
   :status: implemented
   :component: blip-kit
   :satisfies: REQ_LIB_EXAMPLE, REQ_LIB_SOURCE_OWN, REQ_LIB_TABLE_COLUMNS

   A small application in the repository that uses ``blip-kit`` without the
   ``workbench`` feature: a fixed dashboard fed by a simulated source in its
   own process.

.. arch:: outpost
   :id: ARCH_OUTPOST
   :status: implemented
   :component: outpost
   :satisfies: REQ_OUTPOST_SOCKET, REQ_OUTPOST_SIM, REQ_OUTPOST_PROTOCOL, REQ_OUTPOST_BATCH, REQ_OUTPOST_TRACERS, REQ_OUTPOST_LOGGING, REQ_CONN_WS, REQ_CTRL_ANNOUNCE, REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK, REQ_PROTO_NAMES, REQ_CONN_TIMELINE, REQ_FILE_VERSION, REQ_OUTPOST_HELLO, REQ_OUTPOST_METADATA, REQ_OUTPOST_SAMPLES

   Python library ``blip-outpost`` (``outpost/src/blip_outpost``) managed
   with ``uv``, see :doc:`/outpost/internals`. The user's script talks to the target and
   calls the public API exported by ``blip_outpost/__init__.py``.

   * ``outpost.py`` -- ``Outpost``, which serves blip from a thread of its own
     or from the running event loop, takes messages from any thread, alone or
     in batches, and holds the tracers and the actions declared with
     ``@action``.
   * ``tracer.py``, ``records.py`` -- ``Tracer``, named like a logger, the
     bridge from Python's ``logging``, and the metadata of dataclass records.
   * ``actions.py`` -- the ``Actions`` registry, which takes the parameters of
     actions from the type hints of their functions, checks the arguments and
     runs the invocations of an action one after another.
   * ``server.py`` -- ``OutpostServer``, an asyncio WebSocket server that
     broadcasts messages to all connected clients, sends the ``Hello``, the
     metadata and the list of actions to clients connecting later, and passes the invocations
     of each client on, answering that client and logging to all.
   * ``passthrough.py``, ``replay.py`` -- the TCP/UDP passthrough and the
     replay of recordings.
   * ``main.py`` -- ``click`` based command line (``blip-outpost``) for the
     passthrough and the replay.

   The ``minimal``, ``app`` (a program that traces itself) and ``simulator``
   scripts (the simulator also the reference) and ``fast`` (the stress test)
   are examples in
   ``outpost/examples``, outside the library.

.. arch:: sonde
   :id: ARCH_SONDE
   :status: implemented
   :component: sonde
   :satisfies: REQ_PROTO_CBOR, REQ_PROTO_TYPES, REQ_SONDE_LOG, REQ_PROTO_NAMES, REQ_FILE_VERSION, REQ_SONDE_HELLO, REQ_SONDE_METADATA, REQ_SONDE_SAMPLES

   C++17 library (``sonde``) for firmware and unit tests, see :doc:`/sonde/internals`.
   Encodes ``proto`` messages with tinycbor into batches of whole messages, a
   lock-free single-producer single-consumer queue per context, for the
   firmware to send to the outpost's socket passthrough, or for a sink to
   write as a recording file.

   * ``include/sonde/sonde.hpp`` -- ``Queue``, ``Tracer`` (a named part of the
     code on a queue, as the tracers of the outpost), ``Storage``, ``Sink`` and
     the encoding of values, blocks of samples and metadata, and the ``Hello``
     each queue sends first.
   * ``include/sonde/file_sink.hpp`` -- ``FileSink`` and ``MemorySink``
     (hosted builds).
   * ``src/`` -- ``queue.cpp``, ``tracer.cpp``, ``encoder.cpp``, ``logf.cpp``, ``file_sink.cpp``.

Component traceability
----------------------

.. needflow::
   :types: arch, req
   :link_types: satisfies
   :show_link_names:
