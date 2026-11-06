How it fits together
====================

blip shows what an embedded target does. Between the target and blip sits the
**outpost**, which speaks the target's language on one side and blip's on the
other. Firmware and unit tests can also speak blip's language themselves, with
**sonde**.

.. figure:: /diagrams/system-context.svg
   :target: ../_images/system-context.svg
   :class: diagram
   :alt: System context: the embedded target with firmware and optionally
         sonde, the outpost with your script, WebSocket server and recorder,
         blip as web or native app, and recording files written by the
         outpost, by unit tests with sonde and by blip.

   The parts and how they talk.

The parts
---------

.. list-table::
   :header-rows: 1
   :widths: 14 43 43

   * - Part
     - What it is
     - What you do with it
   * - :doc:`blip </blip/index>`
     - The app you look at, in the browser or native (Rust, egui). It connects
       to outposts over WebSockets.
     - Arrange views, record, inspect the data over time, save and open
       recordings and presets, carry out actions.
   * - :doc:`outpost </outpost/index>`
     - A Python library and command, ``blip-outpost``, that runs on the machine
       the target is attached to and serves blip.
     - Write a script that reads your target (serial port, CAN, a socket, a
       vendor library) and sends what it decodes; offer the target's actions;
       pass through a target that speaks blip's protocol; record; replay.
   * - :doc:`sonde </sonde/index>`
     - A C++17 library for firmware and unit tests, without heap or locks.
     - Trace values and log records on the target and send them, e.g. over
       UDP, to an outpost; or write recording files from unit tests that blip
       opens.

They exchange **messages**: values, units, log records, the list of actions and
the answers to invocations, as CBOR over WebSockets (see
:doc:`/reference/protocol`). The same messages, as received, make up a
**recording file**, whoever writes it: blip, the outpost or sonde.
:doc:`concepts` explains the ideas they share.

Which setup you need
--------------------

.. figure:: /diagrams/setups.svg
   :target: ../_images/setups.svg
   :class: diagram
   :alt: Five ways to feed blip: the simulator, your target through your own
         protocol, a target with sonde through the TCP or UDP passthrough, a
         recording written by a unit test, and a replayed recording.

   Ways to feed blip.

.. list-table::
   :header-rows: 1
   :widths: 40 60

   * - Your situation
     - Setup
   * - No hardware at hand, or trying blip out
     - The outpost's simulator, see :doc:`/start/quickstart`.
   * - A target with a protocol of its own (text lines, binary frames, CAN)
     - A Python script with the outpost that reads and decodes it, see
       :doc:`/outpost/index`.
   * - Firmware you can change, with a network link or a UART
     - sonde in the firmware, and ``blip-outpost tcp`` or ``udp`` to pass its
       messages on, see :doc:`/sonde/index`.
   * - A target that sends protobuf messages
     - ``blip-outpost protobuf`` with the annotated schema, see
       :doc:`/reference/outpost-cli`.
   * - Unit tests or a desktop program
     - sonde with a ``FileSink``: blip opens the recording file, no outpost
       needed.
   * - A recording to look at again
     - Open it in blip, or replay it with ``blip-outpost replay`` at its pace.

Where things run
----------------

* blip runs in a browser (served statically, e.g. the hosted app on GitHub
  Pages) or natively. Both connect out to the outposts; nothing serves blip's
  data but the outposts.
* An outpost runs per target, usually on the machine the target is attached
  to. One blip can connect to several outposts, and several blips to one
  outpost.
* Tests and desktop programs write recordings with sonde, without an outpost.

Design principles
-----------------

The outpost sits next to the target
   The link to an embedded target is slow and specific: UART, CAN, a
   proprietary TCP protocol. The outpost translates it on a machine nearby and
   blip only ever speaks one protocol, which favours clarity over bytes;
   compact encodings belong in the outpost's protocol for the target. sonde
   lets a target speak the blip messages directly when that is good enough.

The messages are the data
   blip keeps the received messages byte for byte as a recording; a saved
   recording, an outpost recording and a sonde file are the same format and
   replay as if received. Opening a file runs the same code as receiving.

Plain, self-describing values
   A value is a plain CBOR item: its kind follows from the CBOR type, records
   split into a trace per field. Targets need no schema up front.

One time range, many views
   All views show the same window of time and the same time cursor, like a
   dashboard. The layout is yours, kept in presets apart from the data.

Actions only on request
   blip never invokes an action on its own. The target decides whether to
   carry an invocation out and says so; the readback shows the effect.

In the repository
-----------------

.. list-table::
   :header-rows: 1
   :widths: 15 15 70

   * - Directory
     - Language
     - Contents
   * - ``blip/``
     - Rust
     - The app: web (WASM via Trunk) and native, built on egui/eframe. See
       :doc:`/blip/internals`.
   * - ``proto/``
     - Rust
     - The wire types (``Message``, ``Data``, ``Request``,
       ``ActionDescriptor``), serialised with serde to CBOR: the single source
       of truth of the messages, mirrored by the outpost and sonde.
   * - ``outpost/``
     - Python
     - The library ``blip_outpost``, the ``blip-outpost`` command, the examples
       and the tests. See :doc:`/outpost/internals`.
   * - ``sonde/``
     - C++17
     - The tracing library, its examples and tests. See
       :doc:`/sonde/internals`.
   * - ``doc/``
     - Sphinx
     - This documentation, with the requirements and their traceability
       (:doc:`/requirements/index`).
