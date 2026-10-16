Components
==========

.. arch:: blip frontend
   :id: ARCH_BLIP
   :status: implemented
   :component: blip
   :satisfies: REQ_VIS_PLOT, REQ_VIS_LOG, REQ_VIS_STATES, REQ_VIS_TABLE, REQ_VIS_XY, REQ_VIS_HEATMAP, REQ_VIS_CURSOR, REQ_CONN_WS, REQ_PLAT_WEB, REQ_PLAT_NATIVE, REQ_PERSIST_STATE, REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK, REQ_CTRL_VIEW, REQ_CTRL_SAFE

   egui/eframe application compiled to WebAssembly (served by Trunk) or to a
   native binary. Main modules in ``blip/src``:

   * ``app.rs`` -- application state, WebSocket connection handling and the
     panels of the window.
   * ``app/proto_parser.rs`` -- decodes binary CBOR frames into ``proto``
     messages and feeds the model.
   * ``app/model`` -- data model: value ``traces``, ``logs`` and the
     announced ``actions`` with the ``Invoker``, which sends invocations and
     tracks their answers.
   * ``app/time.rs`` -- the time range shared by all views.
   * ``app/layout`` -- the layout of the views as a tree of groups, shown
     with ``egui_tiles``, the layout panel to edit it and its versioned
     saved form.
   * ``app/view`` -- views: time-series ``plot``, ``states`` over time,
     ``table`` of values, ``xy`` plot, ``heatmap`` of arrays,
     ``log_table`` and the ``actions`` form; ``time_axis`` shared by the views
     over time, and the ``trace_selection`` of the selected view.

.. arch:: proto crate
   :id: ARCH_PROTO
   :status: implemented
   :component: proto
   :satisfies: REQ_PROTO_CBOR, REQ_CTRL_ANNOUNCE

   Rust crate defining the serde-serialisable wire types. It is the single
   source of truth for the message schema; the bridge mirrors it in
   ``bridge/src/bridge/proto.py``.

.. arch:: bridge
   :id: ARCH_BRIDGE
   :status: implemented
   :component: bridge
   :satisfies: REQ_BRIDGE_SOCKET, REQ_BRIDGE_SIM, REQ_BRIDGE_PROTOCOL, REQ_CONN_WS, REQ_CTRL_ANNOUNCE, REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK

   Python package (``bridge/src/bridge``) managed with ``uv``. It is a
   framework: the protocol that talks to a target is provided by the user and
   only uses the public API exported by ``bridge/__init__.py``.

   * ``protocol.py`` -- ``Protocol``, the base class of protocols: ``run``
     talks to the target, ``send_*`` send to blip, ``invoke`` carries out
     invocations.
   * ``actions.py`` -- ``@action`` and the ``Actions`` registry, which takes
     the parameters of actions from the type hints of their functions, checks
     the arguments and runs the invocations of an action one after another.
   * ``server.py`` -- ``BridgeServer``, an asyncio WebSocket server that
     broadcasts protocol messages to all connected clients, sends the list of
     actions to clients connecting later, and passes the invocations of each
     client to the protocol, answering that client and logging to all.
   * ``loading.py`` -- loads the protocol named with ``--protocol`` from a
     file or module and converts the ``--option`` values by the type hints of
     its constructor.
   * ``protocols/`` -- the built-in ``simulator`` (also the reference
     protocol), ``replay`` and ``socket`` (TCP/UDP passthrough).
   * ``main.py`` -- ``click`` based command line entry point (``bridge``) and
     ``run`` to serve a protocol from a script.

Component traceability
----------------------

.. needflow::
   :types: arch, req
   :link_types: satisfies
   :show_link_names:
