Components
==========

.. arch:: blip frontend
   :id: ARCH_BLIP
   :status: implemented
   :component: blip
   :satisfies: REQ_VIS_PLOT, REQ_VIS_LOG, REQ_VIS_STATES, REQ_VIS_TABLE, REQ_VIS_XY, REQ_VIS_HEATMAP, REQ_VIS_CURSOR, REQ_CONN_WS, REQ_PLAT_WEB, REQ_PLAT_NATIVE, REQ_PERSIST_STATE

   egui/eframe application compiled to WebAssembly (served by Trunk) or to a
   native binary. Main modules in ``blip/src``:

   * ``app.rs`` -- application state, WebSocket connection handling and the
     panels of the window.
   * ``app/proto_parser.rs`` -- decodes binary CBOR frames into ``proto``
     messages and feeds the model.
   * ``app/model`` -- data model: value ``streams`` and ``logs``.
   * ``app/time.rs`` -- the time range shared by all views.
   * ``app/layout`` -- the layout of the views as a tree of groups, shown
     with ``egui_tiles``, the layout panel to edit it and its versioned
     saved form.
   * ``app/view`` -- views: time-series ``plot``, ``states`` over time,
     ``table`` of values, ``xy`` plot, ``heatmap`` of arrays and
     ``log_table``; ``time_axis`` shared by the views over time, and the
     ``stream_selection`` of the selected view.

.. arch:: proto crate
   :id: ARCH_PROTO
   :status: implemented
   :component: proto
   :satisfies: REQ_PROTO_CBOR

   Rust crate defining the serde-serialisable wire types. It is the single
   source of truth for the message schema; the bridge mirrors it in
   ``bridge/src/bridge/proto.py``.

.. arch:: bridge
   :id: ARCH_BRIDGE
   :status: implemented
   :component: bridge
   :satisfies: REQ_BRIDGE_SOCKET, REQ_BRIDGE_SIM, REQ_CONN_WS

   Python package (``bridge/src/bridge``) managed with ``uv``:

   * ``server.py`` -- ``BridgeServer``, an asyncio WebSocket server that
     broadcasts protocol messages to all connected clients and forwards
     client actions to the protocol backend.
   * ``protocols/base.py`` -- ``BaseProtocol``, the extension point for new
     device protocols.
   * ``protocols/`` -- ``socket`` (TCP/UDP), ``simulator`` and ``robby``
     (WebSocket client to the robby ESP32, translating robby messages to
     blip messages) implementations.
   * ``main.py`` -- ``click`` based command line entry point (``bridge``).

Component traceability
----------------------

.. needflow::
   :types: arch, req
   :link_types: satisfies
   :show_link_names:
