Overview
========

blip consists of three programs and a shared message schema. They talk CBOR:
blip and the outpost over a WebSocket, the outpost and a target in whatever the
target speaks -- its own protocol, or the blip messages themselves, as encoded
by sonde.

.. figure:: /diagrams/system-context.svg
   :target: ../_images/system-context.svg
   :class: diagram
   :alt: System context: the embedded target with firmware and optionally
         sonde, the outpost with its protocol, WebSocket server and recorder,
         blip as web or native app, and recording files written by the
         outpost, by unit tests with sonde and by blip.

   System context.

.. list-table:: Building blocks
   :header-rows: 1
   :widths: 15 15 70

   * - Component
     - Language
     - Responsibility
   * - ``blip``
     - Rust
     - Web (WASM via Trunk) and native GUI built on egui/eframe. Connects to
       any number of WebSocket endpoints, decodes the messages into traces,
       logs and actions, shows them in views laid out by the user, records and
       saves them, and sends invocations of actions.
   * - ``proto``
     - Rust
     - The wire types (``Message``, ``Data``, ``Request``,
       ``ActionDescriptor``), serialised with serde to CBOR. The single source
       of truth of the schema between the outpost and blip.
   * - ``outpost``
     - Python
     - Asynchronous service next to a target: runs a protocol that talks to
       the target, serves its messages to blip, records them, and carries out
       the actions blip invokes. Comes with a simulator, a replay and a TCP/UDP
       passthrough.
   * - ``sonde``
     - C++17
     - Tracing library for firmware and unit tests: encodes blip messages
       into batches without heap or locks, for the firmware to send or a sink
       to write to a recording file.

Design principles
-----------------

The outpost sits next to the target
   The link to an embedded target is slow and specific: UART, CAN, a
   proprietary TCP protocol. The outpost translates it on a machine nearby and
   blip only ever speaks one protocol. ``proto`` describes the outpost → blip
   link, so it favours clarity over bytes; compact encodings belong in the
   outpost's protocol for the target. sonde lets a target speak the blip
   messages directly when that is good enough.

The messages are the data
   blip keeps the received messages byte for byte as a recording; a saved
   recording, an outpost recording and a sonde file are the same format and
   replay as if received. Opening a file runs the same code as receiving.

Plain, self-describing values
   A value is a plain CBOR item: its kind follows from the CBOR major type,
   records split into a trace per field. Targets need no schema up front.

One time range, many views
   All views show the same window of time and the same time cursor, like a
   dashboard. The layout is the user's, saved apart from the data.

Actions only on request
   blip never invokes an action on its own. The target decides whether to
   carry an invocation out and says so; the readback shows the effect.

Deployments
-----------

.. figure:: /diagrams/setups.svg
   :target: ../_images/setups.svg
   :class: diagram
   :alt: Five setups: simulator, own protocol, sonde through TCP or UDP,
         recording files from unit tests, replay.

   Ways the parts are combined.

* blip runs in a browser (served statically, e.g. GitHub Pages) or natively.
  Both connect out to the outposts; nothing serves blip's data but the
  outposts.
* An outpost runs per target, usually on the machine the target is attached
  to. One blip can connect to several outposts, and several blips to one
  outpost.
* Tests and desktop programs write recordings with sonde, without an outpost.

The following pages describe the shared :doc:`concepts`, the wire
:doc:`protocol`, and the inside of :doc:`blip`, the :doc:`outpost` and
:doc:`sonde`. :doc:`components` traces the components to the requirements.
