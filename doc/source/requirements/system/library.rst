Library
=======

blip split into a library for tracing UIs and the reference app built on it.

The work is phased, and the tag of each requirement tells its phase
(``phase1`` to ``phase6``). The phases 1 to 4 are ``implemented``. Of phase 6,
the crates are prepared and packaged but not published, so its requirement is
still ``draft``, as is the optional InfluxDB store of phase 5. The requirements of
the typed entry of the store and of the connections as sources of the session
are on :doc:`ingest`.

.. list-table::
   :header-rows: 1
   :widths: 10 90

   * - Phase
     - Content
   * - 1
     - The mechanical split into ``blip-proto``, ``blip-kit`` with its
       features and the ``blip`` app, with no change of behaviour.
   * - 2
     - ``Source`` trait, WebSocket and own sources, session.
   * - 3
     - ``Store`` trait and the in-memory store behind it.
   * - 4
     - Widget API, own widgets and columns, registry of view types, example
       application.
   * - 5
     - InfluxDB store (optional).
   * - 6
     - Publishing on crates.io.

.. req:: Library and reference app crates
   :id: REQ_LIB_CRATES
   :status: implemented
   :component: blip, blip-kit, blip-proto
   :priority: medium
   :tags: phase1
   :satisfies: STK_OWN_UI, STK_OWN_DATA

   The software shall consist of the crates ``blip-proto`` (the wire types),
   ``blip-kit`` (the library) and ``blip`` (the reference app, a binary).
   ``blip-kit`` shall use ``blip-proto`` and neither of them shall depend on
   ``blip``.

.. req:: Wire types in blip-proto
   :id: REQ_LIB_PROTO_CRATE
   :status: implemented
   :component: blip-proto
   :priority: medium
   :tags: phase1
   :satisfies: STK_OWN_UI
   :rationale: The wire types were the ``proto`` crate; a library user needs them
      under a name that suits crates.io.

   The wire types shall be in the crate ``blip-proto``, the former ``proto``
   crate renamed, with the wire format unchanged.

.. req:: Features of blip-kit
   :id: REQ_LIB_FEATURES
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase1
   :satisfies: STK_OWN_UI, STK_OWN_DATA

   ``blip-kit`` shall be one crate with these features: ``widgets`` (on by
   default), with the theme, the time range controls, the timeline, all views
   and the action widgets, the tree to choose the traces of a view and the
   registry of view types, needing egui; ``workbench``, with the
   user-configurable layout and the properties of its views, the presets, the list of connections, the toasts and the file
   dialogs, needing ``widgets``; and ``websocket`` (on by default), with the
   WebSocket source. The store, the in-memory store, the recording files, the
   invoker of actions and the session shall be present whatever the features.

.. req:: Library without a GUI
   :id: REQ_LIB_NO_GUI
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase1
   :satisfies: STK_OWN_DATA
   :rationale: A program that only stores or relays data, or a store in a service,
      needs no GUI.

   ``blip-kit`` shall compile with no default features and without egui, and
   with no GUI crate among its dependencies then.

.. req:: Library on web and native
   :id: REQ_LIB_PLATFORMS
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase1
   :satisfies: STK_OWN_UI, STK_ZERO_INSTALL

   ``blip-kit`` and an application built on it shall build for the web
   (WebAssembly) and for native desktop, as blip does, with each feature that
   applies to the target.

.. req:: The shipped blip is the reference app
   :id: REQ_LIB_REFERENCE
   :status: implemented
   :component: blip
   :priority: medium
   :tags: phase1
   :satisfies: STK_OWN_UI
   :rationale: What blip does must be possible for any application built on the
      library.

   The ``blip`` crate shall be a binary of a ``main.rs`` and a thin ``App``
   that uses ``blip-kit`` with all its features, and shall hold no views,
   store or protocol logic of its own.

.. req:: No change for the blip user
   :id: REQ_LIB_NO_CHANGE
   :status: implemented
   :component: blip
   :priority: high
   :tags: phase1
   :satisfies: STK_LIVE_OBSERVE
   :rationale: The split is a restructuring; the user of blip shall not notice it.

   blip shall behave and look as before the restructuring of the library, in
   each phase of it.

.. req:: Presets and recordings keep loading
   :id: REQ_LIB_COMPAT
   :status: implemented
   :component: blip, blip-kit
   :priority: high
   :tags: phase1
   :satisfies: STK_REVIEW

   blip shall load presets, layout files and recording files written before
   the restructuring unchanged, and shall keep doing so in each later phase.

.. req:: Sources
   :id: REQ_LIB_SOURCE
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase2
   :satisfies: STK_OWN_DATA

   ``blip-kit`` shall define a ``Source`` trait. When the session polls it, a
   source shall hand over what it has to an inbox it is given: the bytes of
   blip-protocol messages as they are, one message or several, which the
   library reads, or typed data of its own (samples, blocks of samples, log
   records, trace metadata, announced actions, its ``Hello`` and the results of
   actions), without encoding it and without building protocol messages. A
   source shall tell when it opened, closed or failed, and take requests, such
   as the invocation of an action. An application shall be able to implement it
   for its own data, such as a serial port, UDP or a simulator, whether that
   data is the blip protocol or not.

.. req:: WebSocket source
   :id: REQ_LIB_SOURCE_WS
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase2
   :satisfies: STK_DEVICE_IF

   With the ``websocket`` feature ``blip-kit`` shall provide a source for
   the WebSocket connection to an outpost, behaving as the connections of blip
   do now: it hands the binary messages it receives over as they are, and
   tells when the connection opened, closed or failed.

.. req:: Own source in the same process
   :id: REQ_LIB_SOURCE_OWN
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase2
   :satisfies: STK_OWN_DATA
   :rationale: No separate outpost and no socket shall be needed to show data of the
      program itself.

   An application shall be able to show data from a source of its own in its
   own process, without a WebSocket connection or an outpost, with the traces,
   logs and actions of that source shown as those of any other, though its
   data never was CBOR or the blip protocol.

.. req:: Session
   :id: REQ_LIB_SESSION
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase2
   :satisfies: STK_OWN_UI, STK_OWN_DATA

   ``blip-kit`` shall provide a session that, once per frame, polls its sources,
   takes what they hand over into a store and sends the pending requests to
   them, and shall need no GUI to do so. The store shall be the in-memory store
   or a store of the application that takes the typed entry
   (:need:`REQ_LIB_INGEST_TYPED`) and answers the queries of the ``Store``
   trait. Whatever the store, the session shall record what arrives while
   recording (:need:`REQ_FILE_REENCODE`) and hand the results of actions to the
   invoker.

.. req:: Store trait
   :id: REQ_LIB_STORE
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase3
   :satisfies: STK_OWN_DATA

   ``blip-kit`` shall define a ``Store`` trait with windowed queries by
   path, time window and maximum number of points or resolution. A query shall
   be synchronous and shall not block, and each answer shall tell whether more
   data is pending.

.. req:: Views read through the store
   :id: REQ_LIB_STORE_ONLY
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase3
   :satisfies: STK_OWN_DATA

   The views shall read samples only through the ``Store`` trait, and not
   depend on the kind of store.

.. req:: In-memory store keeps its speed
   :id: REQ_LIB_STORE_MEMORY
   :status: implemented
   :component: blip-kit
   :priority: high
   :tags: phase3
   :satisfies: STK_LIVE_OBSERVE
   :rationale: Neither the trait nor the typed entry must cost the user of blip
      anything.

   The in-memory store shall implement ``Store`` and take the typed entry
   (:need:`REQ_LIB_INGEST_TYPED`), and shall not be slower for the user than
   before: the rate at which it takes in samples, while recording and while not
   recording, single samples and blocks of samples, and the frame time of a plot
   with tens of millions of samples shall stay within 10 % of what they were
   before the typed entry.

.. req:: InfluxDB store
   :id: REQ_LIB_STORE_INFLUX
   :status: draft
   :component: blip-kit
   :priority: low
   :tags: phase5
   :satisfies: STK_OWN_DATA
   :rationale: A second implementation proves that the trait does not only fit the
      memory. Optional, planned for later.

   ``blip-kit`` may provide a ``Store`` that reads its samples from
   InfluxDB, shown by the same views.

.. req:: Widgets on their own
   :id: REQ_LIB_WIDGETS
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase4
   :satisfies: STK_OWN_UI

   With the ``widgets`` feature, each of the plot, XY plot, states, heatmap,
   distribution, histogram, stats, table and log table views, the action
   widgets, the timeline and the time range controls shall be usable on its
   own in an egui UI, without the ``workbench``, and configured in code, such
   as the traces it shows.

.. req:: Widgets of one's own
   :id: REQ_LIB_WIDGETS_OWN
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase4
   :satisfies: STK_OWN_UI

   An application shall be able to write its own widgets that read the store,
   the time range and the theme through the public API of ``blip-kit`` as
   the widgets of the library do, and use them next to those.

.. req:: Own columns in the table
   :id: REQ_LIB_TABLE_COLUMNS
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase4
   :satisfies: STK_OWN_UI

   The table shall accept columns defined by the application that draw
   controls, such as a button, and invoke actions.

.. req:: Actions from any widget
   :id: REQ_LIB_ACTIONS
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase4
   :satisfies: STK_OWN_UI, STK_CONTROL

   An application shall be able to invoke an announced action from any of its
   widgets through the invoker, and to show the progress and answer of the
   invocation, whether or not an action widget is shown.

.. req:: Registry of view types
   :id: REQ_LIB_REGISTRY
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase4
   :satisfies: STK_OWN_UI

   An application shall be able to register a view type of its own beside
   those of the library, so that it can be added to the configurable layout of
   the ``workbench``, and the layout shall save such views in presets and load
   them again.

.. req:: Example application
   :id: REQ_LIB_EXAMPLE
   :status: implemented
   :component: blip-kit
   :priority: medium
   :tags: phase4
   :satisfies: STK_OWN_UI

   The repository shall hold an example application, a fixed dashboard of a
   plot, a table with a button column and the timeline, fed by a simulated
   source in its own process, without the traces panel and the properties
   panel. The checks of the repository (``check.sh``) shall build it for native
   and for the web.

.. req:: Published on crates.io
   :id: REQ_LIB_PUBLISH
   :status: draft
   :component: blip-kit, blip-proto
   :priority: medium
   :tags: phase6
   :satisfies: STK_OWN_UI

   ``blip-proto`` and ``blip-kit`` shall be published on crates.io under
   semantic versioning, with the metadata and documentation crates.io shows,
   and the publishing of each shall pass a dry run in the continuous
   integration.
