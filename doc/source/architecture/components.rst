Components
==========

.. arch:: blip frontend
   :id: ARCH_BLIP
   :status: implemented
   :component: blip
   :satisfies: REQ_VIS_PLOT, REQ_VIS_LOG, REQ_VIS_STATES, REQ_VIS_TABLE, REQ_VIS_XY, REQ_VIS_HEATMAP, REQ_VIS_CURSOR, REQ_CONN_WS, REQ_PLAT_WEB, REQ_PLAT_NATIVE, REQ_PERSIST_STATE, REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK, REQ_CTRL_VIEW, REQ_CTRL_SAFE

   egui/eframe application compiled to WebAssembly (served by Trunk) or to a
   native binary. Main modules in ``blip/src``, see :doc:`blip`:

   * ``app.rs`` -- the panels of the window, the menu, files and persistence,
     and the ``State`` of time range, recording, model and invoker.
   * ``app/connections.rs`` -- the WebSocket connections and their panel;
     splits frames into messages.
   * ``app/model`` -- the data model: value ``traces``, ``logs``, the
     announced ``actions`` with the ``Invoker``, which sends invocations and
     tracks their answers, and the ``recording`` of the received messages.
   * ``app/time.rs`` -- the time range shared by all views, and the timeline.
   * ``app/layout`` -- the layout of the views as a tree of groups, shown
     with ``egui_tiles``, the layout and properties panels and its versioned
     saved form.
   * ``app/view`` -- views: time-series ``plot``, ``states`` over time,
     ``table`` of values, ``xy`` plot, ``heatmap`` of arrays,
     ``log_table`` and the ``actions`` form; ``time_axis`` shared by the views
     over time, and the ``trace_selection``, the tree of the traces and
     actions to choose those of the selected view.

.. arch:: proto crate
   :id: ARCH_PROTO
   :status: implemented
   :component: proto
   :satisfies: REQ_PROTO_CBOR, REQ_CTRL_ANNOUNCE

   Rust crate defining the serde-serialisable wire types. It is the single
   source of truth for the message schema; the outpost mirrors it in
   ``outpost/src/outpost/proto.py``.

.. arch:: outpost
   :id: ARCH_OUTPOST
   :status: implemented
   :component: outpost
   :satisfies: REQ_OUTPOST_SOCKET, REQ_OUTPOST_SIM, REQ_OUTPOST_PROTOCOL, REQ_CONN_WS, REQ_CTRL_ANNOUNCE, REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK

   Python package (``outpost/src/outpost``) managed with ``uv``, see
   :doc:`outpost`. It is a
   framework: the protocol that talks to a target is provided by the user and
   only uses the public API exported by ``outpost/__init__.py``.

   * ``protocol.py`` -- ``Protocol``, the base class of protocols: ``run``
     talks to the target, ``send_*`` send to blip, ``invoke`` carries out
     invocations.
   * ``actions.py`` -- ``@action`` and the ``Actions`` registry, which takes
     the parameters of actions from the type hints of their functions, checks
     the arguments and runs the invocations of an action one after another.
   * ``server.py`` -- ``OutpostServer``, an asyncio WebSocket server that
     broadcasts protocol messages to all connected clients, sends the list of
     actions to clients connecting later, and passes the invocations of each
     client to the protocol, answering that client and logging to all.
   * ``loading.py`` -- loads the protocol named with ``--protocol`` from a
     file or module and converts the ``--option`` values by the type hints of
     its constructor.
   * ``protocols/`` -- the built-in ``simulator`` (also the reference
     protocol), ``replay`` and ``socket`` (TCP/UDP passthrough).
   * ``main.py`` -- ``click`` based command line entry point (``outpost``) and
     ``run`` to serve a protocol from a script.

.. arch:: sonde
   :id: ARCH_SONDE
   :status: implemented
   :component: sonde
   :satisfies: REQ_PROTO_CBOR, REQ_PROTO_TYPES

   C++17 library (``sonde``) for firmware and unit tests, see :doc:`sonde`.
   Encodes ``proto`` messages with tinycbor into batches of whole messages, a
   lock-free single-producer single-consumer queue per tracer, for the
   firmware to send to the outpost's socket passthrough, or for a sink to
   write as a recording file.

   * ``include/sonde/sonde.hpp`` -- ``Tracer``, ``Storage``, ``Sink`` and the
     encoding of values.
   * ``include/sonde/file_sink.hpp`` -- ``FileSink`` and ``MemorySink``
     (hosted builds).
   * ``src/`` -- ``tracer.cpp``, ``encoder.cpp``, ``file_sink.cpp``.

Component traceability
----------------------

.. needflow::
   :types: arch, req
   :link_types: satisfies
   :show_link_names:
