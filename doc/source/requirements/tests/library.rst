Library
=======

Tests of the requirements in :doc:`/requirements/system/library`. The tests
carry the tag of the phase of their requirements. Their locations are filled
in by the developer of each phase; a test whose location says so is done in
part or by hand.

.. test:: The workspace has the three crates
   :id: TEST_LIB_CRATES
   :status: implemented
   :component: blip, blip-kit, blip-proto
   :tags: phase1
   :verifies: REQ_LIB_CRATES, REQ_LIB_PROTO_CRATE

   ``check.sh``, ``proto/tests`` -- ``check.sh`` builds the workspace of the three crates and packages ``blip-proto`` and ``blip-kit`` as they would be published.

   Checked by review of the manifests, not by a test: checks with ``cargo
   metadata`` that the workspace has the packages ``blip-proto``, ``blip-kit``
   and ``blip``, that no ``proto`` package is left, that ``blip`` is the only
   one with a binary, that ``blip-kit`` depends on ``blip-proto`` and that
   nothing depends on ``blip``. The tests of the wire types pass under the new
   name and encode the same bytes as before.

.. test:: Features can be chosen and depend on each other
   :id: TEST_LIB_FEATURES
   :status: implemented
   :component: blip-kit
   :tags: phase1
   :verifies: REQ_LIB_FEATURES

   ``check.sh`` -- builds ``blip-kit`` with no features natively and for the web, with the default features, and with all features through the workspace; the tests run with all features. The features are not built one by one.

   Planned and not done: checks that ``blip-kit`` builds, with its tests, for
   the default features, for each of ``widgets``, ``workbench`` and
   ``websocket`` alone and for the combinations, that ``workbench`` turns on
   ``widgets``, and that the store, the in-memory store, the recording files,
   the invoker and the session are present with no default features.

.. test:: Without default features there is no egui
   :id: TEST_LIB_NO_GUI
   :status: implemented
   :component: blip-kit
   :tags: phase1
   :verifies: REQ_LIB_NO_GUI

   ``check.sh`` -- checks that ``blip-kit`` builds with
   ``--no-default-features`` and that ``cargo tree`` for it lists neither
   ``egui`` nor ``eframe``.

.. test:: Library and apps build for web and native
   :id: TEST_LIB_BUILDS
   :status: implemented
   :component: blip, blip-kit
   :tags: phase1
   :verifies: REQ_LIB_PLATFORMS

   ``check.sh`` -- builds ``blip-kit`` for the native target and for
   ``wasm32-unknown-unknown`` with the features that apply to each, and the
   ``blip`` app for both. It is run by the developers; no job of the
   continuous integration runs it.

.. test:: The blip app is thin
   :id: TEST_LIB_REFERENCE
   :status: implemented
   :component: blip
   :tags: phase1
   :verifies: REQ_LIB_REFERENCE

   Review of ``blip/src`` and ``blip/Cargo.toml``, no automated test: the
   sources of ``blip`` are ``main.rs`` and the ``App`` module only, ``blip``
   depends on ``blip-kit`` with the feature ``workbench``, which turns on
   ``widgets`` and ``websocket``, and it holds no module of views, store or
   protocol.

.. test:: Behaviour of blip is unchanged
   :id: TEST_LIB_NO_CHANGE
   :status: implemented
   :component: blip, blip-kit
   :tags: phase1
   :verifies: REQ_LIB_NO_CHANGE

   ``kit/src/**/tests.rs``, ``blip/src/app/tests.rs``, ``make screenshots`` -- the screenshots are compared by hand with ``make screenshots SCREENSHOTS_DIR=…``; after the restructuring all 64 were the same pixel for pixel.

   Planned and not done: the tests of blip from before the restructuring,
   moved with the code they test to ``blip-kit`` or kept in ``blip``, pass
   without change of what they check, and the screenshots of the documentation
   do not differ from those before it.

.. test:: Old presets and recordings load
   :id: TEST_LIB_COMPAT
   :status: implemented
   :component: blip, blip-kit
   :tags: phase1
   :verifies: REQ_LIB_COMPAT

   ``kit/src/workbench/presets/tests.rs``, ``kit/src/workbench/layout/tests.rs``, ``kit/src/store/recording/tests.rs``, ``kit/src/widgets/view/custom/tests.rs`` -- the tests of the presets, layouts and recordings from before the restructuring pass unchanged, and the views of the library are saved as before.

   Planned and not done: loads presets, layout files and recording files of
   the versions 1 to 3 written before the restructuring (kept as test data)
   and checks that they give the same layouts, connections and data as before,
   in each phase. ``kit/src/store/recording/tests.rs::kept_recordings_show_the_same_data``
   also opens the recordings of versions 1 (``blip/tests/data/demo.blip``), 2 (made from it
   with the names of version 2) and 3 (``sonde/tests/data/recording.blip``) and the outpost's,
   and compares a digest of their traces, units, logs and actions with the one taken before
   the typed entry.

.. test:: A source delivers messages and takes requests
   :id: TEST_LIB_SOURCE
   :status: implemented
   :component: blip-kit
   :tags: phase2
   :verifies: REQ_LIB_SOURCE, REQ_LIB_SESSION

   ``kit/src/store/session/tests.rs`` -- a source of the test delivers frames of one and of several messages and takes the invocations.

   ``kit/src/store/session/tests.rs`` (``a_store_of_the_application_takes_typed_data``,
   ``a_frame_may_hold_several_messages``, ``a_source_that_ends_is_dropped_with_a_notice``,
   ``invocations_go_to_the_source_that_announced_the_action``)
   -- implements ``Source`` in a test, once handing over the bytes of
   protocol messages and once typed data of each kind with no protocol message
   built, runs a session frame by frame without egui, and checks that what a
   source hands over arrives in the store in order, that a source tells when it
   opened, closed or failed, and that an action invocation reaches the source as
   a request. The same messages as bytes and as typed data giving the same is
   checked by :need:`TEST_LIB_INGEST_SAME`.

.. test:: The WebSocket source behaves as the connections did
   :id: TEST_LIB_SOURCE_WS
   :status: implemented
   :component: blip-kit
   :tags: phase2
   :verifies: REQ_LIB_SOURCE_WS

   ``kit/src/workbench/connections/tests.rs`` -- the tests of the connections of blip, now on the WebSocket
   source, pass: open, messages before the first ``Hello``, errors, newer
   protocol, and invocations sent. ``kit/src/store/websocket/tests.rs`` (``a_websocket_hands_over_its_frames_as_they_are``, ``a_websocket_that_cannot_connect_fails``) also checks that the source hands
   the binary messages of the socket over as they are, one frame as received,
   and tells when the socket opened, closed or failed.

.. test:: Data of the same process is shown without an outpost
   :id: TEST_LIB_SOURCE_OWN
   :status: implemented
   :component: blip-kit
   :tags: phase2
   :verifies: REQ_LIB_SOURCE_OWN

   ``kit/src/store/session/tests.rs``, ``kit/examples/dashboard.rs`` -- a source in the test process delivers traces, a log record and
   an action with no socket open, and checks that the store holds them as if
   an outpost had sent them. ``kit/src/store/session/tests.rs::a_store_of_the_application_takes_typed_data``
   and the simulator of ``kit/examples/dashboard.rs`` (by review: typed calls only) also check that the source of the test
   hands over its data typed, with no CBOR encoded and no protocol message built,
   and that the example application's simulated source does the same.

.. test:: Queries of a store are windowed, non-blocking and tell what is pending
   :id: TEST_LIB_STORE
   :status: implemented
   :component: blip-kit
   :tags: phase3
   :verifies: REQ_LIB_STORE, REQ_LIB_STORE_ONLY

   ``kit/tests/store.rs`` -- a plot reads a store of the test that is not the in-memory one, which says that more is pending and sees that the plot asks for the shown range at the resolution of its width and never for all samples.

   Planned and not done: runs the same checks on the in-memory store and on a
   test store that answers late: a query for a path, a window and a maximum
   number of points returns no more points than asked, returns at once, and
   says that more data is pending until the late store has answered. The views
   are built on a store that is not the in-memory one and show its data.

.. test:: The in-memory store is as fast as before
   :id: TEST_LIB_STORE_MEMORY
   :status: implemented
   :component: blip-kit
   :tags: phase3
   :verifies: REQ_LIB_STORE_MEMORY

   ``kit/tests/baseline.rs`` (ignored, run by hand) -- measured before and after the ``Store`` trait on the same machine: 1.45 and 1.6 million samples a second taken in, 0.74 and 0.71 to 0.75 ms a frame of a plot of 10 million samples.

   Earlier, for the ``Store`` trait: a benchmark of the ingest rate and of the
   frame time of a plot with tens of millions of samples, run before and after
   the trait, with the result after no worse than before beyond the noise of the
   measurement.

   ``kit/tests/baseline.rs`` (``ingest_rate``, ``ingest_unrecorded``, ``ingest_blocks``,
   ``ingest_typed``, ``plot_frame_time``) -- for the typed entry, a measurement by hand with ``kit/tests/baseline.rs``
   (ignored, run on its own) on one machine, before the change and after it:
   the rate of taking in samples while recording (``ingest_rate``), while not
   recording (``ingest_unrecorded``) and of blocks of floats and of small
   records (``ingest_blocks``), and the frame time of a plot of 10 million
   samples (``plot_frame_time``). Each of the four after the change shall be
   within 10 % of its value before. The numbers of both runs are written here. The
   rate of single samples handed over typed (``ingest_typed``) is
   measured for information.

   Measured on this machine, quiet runs only, with
   ``cargo test --release -p blip-kit --test baseline -- --ignored --nocapture --test-threads=1``.
   Before the change (2026-11-24, two runs), the store fed directly: taking in
   samples while recording 1.69 and 1.67 million samples a second, while not
   recording 6.31 and 6.63, blocks of 100 floats 4.04 and 4.21, blocks of 100
   records 1.06 and 1.07, and a frame of a plot of 10 million samples 0.715 and
   0.807 ms. After the change (2026-11-25, three runs), through the inbox of the
   session, which includes splitting the frame: recording 1.80, 1.85 and 1.86,
   not recording 7.69, 7.72 and 7.91, floats 4.22, 4.45 and 4.34, records 1.44,
   1.49 and 1.46 million samples a second, and the plot 0.710, 0.715 and
   0.675 ms. Each is within 10 % of its value before. Samples handed over typed
   while recording, new: 2.18, 2.27 and 2.34 million samples a second.

.. test:: InfluxDB store answers the store checks
   :id: TEST_LIB_STORE_INFLUX
   :status: draft
   :component: blip-kit
   :tags: phase5
   :verifies: REQ_LIB_STORE_INFLUX

   ``TODO`` -- runs the checks of ``TEST_LIB_STORE`` on the InfluxDB store
   against an InfluxDB instance of the test run, and shows a plot from it.

.. test:: Widgets are used on their own
   :id: TEST_LIB_WIDGETS
   :status: implemented
   :component: blip-kit
   :tags: phase4
   :verifies: REQ_LIB_WIDGETS

   ``kit/src/widgets/**/tests.rs``, ``kit/examples/dashboard.rs``, ``kit/tests/store.rs`` -- the tests of each view show it on its own in a harness; the example shows a plot, a table, the time controls and the timeline made in code.

   Planned and not done: builds an egui UI of one widget of each kind (plot,
   XY plot, states, heatmap, distribution, histogram, stats, table, log table,
   action widget, timeline, time range controls) without the ``workbench``
   feature, configures the traces of each in code, runs frames and checks that
   each draws its data and that none needs the traces panel.

.. test:: Own widgets use the public API
   :id: TEST_LIB_WIDGETS_OWN
   :status: implemented
   :component: blip-kit
   :tags: phase4
   :verifies: REQ_LIB_WIDGETS_OWN

   ``kit/tests/own_view.rs`` -- an integration test outside the crate writes a widget that reads
   the store, the time range and the theme and draws next to a plot, and
   checks that it follows the time range of the plot.

.. test:: The table takes columns with controls
   :id: TEST_LIB_TABLE_COLUMNS
   :status: implemented
   :component: blip-kit
   :tags: phase4
   :verifies: REQ_LIB_TABLE_COLUMNS

   ``kit/src/widgets/view/table/tests.rs``, ``kit/examples/dashboard.rs`` -- a table with a column of the application that draws a button
   per row: the button is drawn in each row and a click invokes an action.

.. test:: Actions are invoked from any widget
   :id: TEST_LIB_ACTIONS
   :status: implemented
   :component: blip-kit
   :tags: phase4
   :verifies: REQ_LIB_ACTIONS

   ``kit/src/widgets/view/table/tests.rs``, ``kit/src/store/session/tests.rs``, ``kit/examples/dashboard.rs`` -- invokes an announced action through the invoker from a widget of
   the test with no action widget in the UI, and checks that the source gets
   the invocation and that the progress and the answer are readable.

.. test:: View types are registered and saved
   :id: TEST_LIB_REGISTRY
   :status: implemented
   :component: blip-kit
   :tags: phase4
   :verifies: REQ_LIB_REGISTRY

   ``kit/src/widgets/view/custom/tests.rs``, ``kit/tests/own_view.rs`` -- registers a view type of the test, adds it to the layout, saves
   the layout as a preset and loads it again with the view in place with its
   settings, and checks that the views of the library load unchanged by the
   registry.

.. test:: The example application builds and runs
   :id: TEST_LIB_EXAMPLE
   :status: implemented
   :component: blip-kit
   :tags: phase4
   :verifies: REQ_LIB_EXAMPLE

   ``check.sh``, ``kit/examples/dashboard.rs`` -- ``check.sh`` builds and tests the example natively and builds it for the web.

   The test of the example runs its frames and checks that its source fed
   the store, that the table has its button column and that a click on a
   button drains the tank of its row.

   Planned and not done: a job of the continuous integration that runs
   ``check.sh``.

.. test:: The crates pass a publish dry run
   :id: TEST_LIB_PUBLISH
   :status: draft
   :component: blip-kit, blip-proto
   :tags: phase6
   :verifies: REQ_LIB_PUBLISH

   ``check.sh`` -- packages both crates with ``cargo package``; nothing is published yet.

   Planned and not done: ``cargo publish --dry-run`` passes for ``blip-proto``
   and ``blip-kit`` in the continuous integration, with the description,
   license, repository and documentation set and the documentation built.
