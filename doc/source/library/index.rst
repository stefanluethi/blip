The library
===========

``blip-kit`` is the library blip is built from. Use it to build your own tracing
UI: take the plots, tables and the timeline of blip and put them in your own
`egui <https://github.com/emilk/egui>`_ application, read the data in the
application itself instead of through an outpost, or keep the data somewhere
else than in memory.

.. admonition:: Before you begin

   A Rust toolchain and some knowledge of egui and eframe. To see what it
   takes, run the example: ``cargo run --release -p blip-kit --example
   dashboard``. How blip itself is put together is in :doc:`/blip/internals`.

The crates
----------

.. list-table::
   :header-rows: 1
   :widths: 22 18 60

   * - Crate
     - Directory
     - Contents
   * - ``blip-proto``
     - ``proto/``
     - The wire types, see :doc:`/reference/protocol`. ``blip-kit``
       re-exports them as ``blip_kit::proto``.
   * - ``blip-kit``
     - ``kit/``
     - The library.
   * - ``blip``
     - ``blip/``
     - The application, the reference of an application built on the library.

``blip-kit`` is one crate with features:

.. list-table::
   :header-rows: 1
   :widths: 22 78

   * - Feature
     - What it adds
   * - (none)
     - ``store``: the traces, logs and actions received, the ``Store`` trait
       the widgets read through, the in-memory ``MemoryStore``, the ``Source``
       trait, the ``Session``, recording files and the invoker of actions.
       It has no GUI crate among its dependencies and builds natively and for
       the web. See :doc:`stores` and :doc:`sources`.
   * - ``websocket`` (default)
     - ``store::websocket``: the WebSocket source, to connect to an outpost or
       a target.
   * - ``widgets`` (default)
     - ``widgets``: the theme, the time range with its controls and timeline,
       and the views. Needs egui. See :doc:`widgets`.
   * - ``workbench``
     - ``workbench``: the layout the user arranges the views in, the presets,
       the connections, the toasts and the file dialogs. Turns on ``widgets``
       and ``websocket``. blip is this and a window around it.
   * - ``test-util``
     - Helpers for the tests of an application, e.g. selecting a view by its
       name. Not for use outside tests.

The parts depend on each other in one direction only: ``workbench`` uses
``widgets`` and ``store``, ``widgets`` uses ``store``, and ``store`` uses
neither.

What an application needs
-------------------------

* A **source** of messages: anything that implements the ``Source`` trait,
  e.g. the ``WebSocket`` source, or your own, which reads a serial port or
  simulates a target. See :doc:`sources`.
* A **session**, which receives from the sources into its store and sends the
  invocations of actions back. See :doc:`sources`.
* A **time range**, the time all widgets show.
* The **widgets** you want, made with the traces they show and drawn where you
  want them. See :doc:`widgets`.

:doc:`app` builds an application from these four, step by step.

Without the workbench, the layout is yours: you decide which widgets there are
and where they are. With the workbench the user arranges the views, and your own
view types can be among them, see :doc:`widgets`. blip itself uses the
workbench: its ``App`` holds the ``Connections`` of the workbench, which keep
the ``WebSocket`` sources and show their list, and a ``Session``.

Web and native
--------------

The library and the applications built on it compile natively and to
WebAssembly. The example builds for both, and ``check.sh`` builds it for the
web too. In the browser a source cannot block: a ``Source`` never does, see
:doc:`sources`.

Limits
------

* No widget shows a loading state yet. ``Store::pending`` and
  ``Loaded::pending`` are there for the application to show one.
* A source delivers CBOR frames. There is no way to hand over decoded messages
  without encoding them; ``Event::message`` encodes a message for a source
  that makes its messages itself.
* The file dialogs of the workbench (``rfd``) need the ``wayland`` feature of
  eframe or the libwayland development files on Linux.
* The crates are prepared for crates.io: they have their metadata and
  READMEs, and ``check.sh`` packages them. They are not published yet. A store
  that keeps its data in InfluxDB is planned, not written.
* egui types are part of the API of the library. A breaking release of egui
  means a new major version of ``blip-kit``.

.. toctree::
   :maxdepth: 1

   app
   widgets
   sources
   stores
