Running
=======

Start the bridge
----------------

The bridge exposes a WebSocket server (default
``ws://127.0.0.1:3000/ws/bridge``) and connects to a data source selected with
``--protocol``. All commands are run from the ``bridge`` directory.

Simulator (no hardware required)::

   uv run bridge --protocol simulator --port 3000

The simulator streams per-core CPU load and an accelerometer value, an ``imu``
record of two 3-element arrays, a ``motor`` record of an integer, a bool and a
string, and a temperature that is ``null`` now and then. It also emits log
records from a few synthetic components at all severity levels. Sustained
high CPU load on a core is reported as a warning.

TCP endpoint::

   uv run bridge --protocol tcp --remote-host 127.0.0.1 --remote-port 9000

UDP endpoint::

   uv run bridge --protocol udp --remote-host 127.0.0.1 --remote-port 9000

Add ``--listen`` to accept connections from the target instead of connecting
to it.

Robby (ESP32 communication module, ``ws://<ip>/ws/robby``)::

   uv run bridge --protocol robby --remote-host 192.168.1.70

The bridge connects to robby as a WebSocket client and translates robby's
messages to blip messages:

* Stream descriptors become value metadata; stream names like ``acc::x`` are
  shown as ``acc/x``.
* Stream frames become one value per sample, scaled with the descriptor's
  scale factor. Sample timestamps are reconstructed from the sampling time,
  anchored to the time the frame arrives. Frames received before the first
  stream descriptor are dropped (robby resends descriptors every second).
* Log records keep their level (fatal is shown as error) and get the
  component ``robby`` and the time they arrive.
* The thread table becomes ``threads/<name>/runtime`` and
  ``threads/<name>/stack_usage`` values in percent.
* Distance maps are not supported by blip and are ignored.

Actions sent by blip are forwarded to robby unchanged. The bridge reconnects
when the connection to robby is lost.

Options:

.. list-table::
   :header-rows: 1
   :widths: 25 20 55

   * - Option
     - Default
     - Description
   * - ``--protocol`` / ``-p``
     - ``simulator``
     - Data source: ``simulator``, ``tcp``, ``udp`` or ``robby``
   * - ``--host`` / ``-h``
     - ``127.0.0.1``
     - WebSocket server bind address
   * - ``--port``
     - ``3000``
     - WebSocket server port
   * - ``--path``
     - ``/ws/bridge``
     - WebSocket endpoint path
   * - ``--remote-host``
     - ``127.0.0.1``
     - Target host (TCP/UDP/robby); bind address with ``--listen``
   * - ``--remote-port``
     - ``9000`` (robby: ``80``)
     - Target port (TCP/UDP/robby); listen port with ``--listen``
   * - ``--remote-path``
     - ``/ws/robby``
     - WebSocket endpoint path of robby
   * - ``--listen``
     - off
     - Listen for the target instead of connecting (TCP/UDP)
   * - ``--interval``
     - ``20``
     - Simulator sampling interval in milliseconds
   * - ``--log-interval``
     - ``500``
     - Mean simulator log interval in milliseconds
   * - ``--verbose`` / ``-v``
     - off
     - Enable debug output

Run ``uv run bridge --help`` for the complete list.

Start blip
----------

Web (development server with live reload)::

   cd blip && trunk serve

then open http://127.0.0.1:8080 in a browser.

Native::

   cargo run --release -p blip
