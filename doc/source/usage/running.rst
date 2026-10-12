Running
=======

Start the bridge
----------------

The bridge exposes a WebSocket server (default
``ws://127.0.0.1:3000/ws/bridge``) and connects to a data source selected with
``--protocol``. All commands are run from the ``bridge`` directory.

Simulator (no hardware required)::

   uv run bridge --protocol simulator --port 3000

The simulator streams per-core CPU load and an accelerometer value, and emits
log records from a few synthetic components at all severity levels. Sustained
high CPU load on a core is reported as a warning.

TCP endpoint::

   uv run bridge --protocol tcp --remote-host 127.0.0.1 --remote-port 9000

UDP endpoint::

   uv run bridge --protocol udp --remote-host 127.0.0.1 --remote-port 9000

Add ``--listen`` to accept connections from the target instead of connecting
to it.

Options:

.. list-table::
   :header-rows: 1
   :widths: 25 20 55

   * - Option
     - Default
     - Description
   * - ``--protocol`` / ``-p``
     - ``simulator``
     - Data source: ``simulator``, ``tcp`` or ``udp``
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
     - Target host (TCP/UDP); bind address with ``--listen``
   * - ``--remote-port``
     - ``9000``
     - Target port (TCP/UDP); listen port with ``--listen``
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
