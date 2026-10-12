Overview
========

blip consists of three parts that communicate over WebSockets using a shared
CBOR-encoded message schema:

.. graphviz::
   :caption: System context

   digraph context {
      rankdir=LR;
      node [shape=box, style="rounded,filled", fillcolor="#f5f5f5", fontname="sans-serif"];
      edge [fontname="sans-serif", fontsize=10];

      target [label="Embedded target\n(firmware)"];
      bridge [label="bridge\n(Python)"];
      blip   [label="blip\n(Rust / egui, web + native)"];
      proto  [label="proto\n(shared schema)", shape=note, fillcolor="#fff8dc"];

      target -> bridge [label="Serial (COBS) /\nTCP / UDP", dir=both];
      bridge -> blip   [label="WebSocket\nCBOR frames", dir=both];
      proto  -> blip   [style=dashed, arrowhead=none];
      proto  -> bridge [style=dashed, arrowhead=none, label="mirrored"];
   }

.. list-table:: Building blocks
   :header-rows: 1
   :widths: 15 15 70

   * - Component
     - Language
     - Responsibility
   * - ``blip``
     - Rust
     - Web (WASM via Trunk) and native GUI built on egui. Connects to a
       WebSocket endpoint, decodes messages and shows plots and log tables.
   * - ``proto``
     - Rust
     - Wire protocol types (``Message``, ``Action``) shared between blip and
       embedded targets.
   * - ``bridge``
     - Python
     - Asynchronous bridge from a device protocol (serial, TCP/UDP,
       simulator) to a WebSocket server.

See :doc:`components` for details and :doc:`protocol` for the message schema.
