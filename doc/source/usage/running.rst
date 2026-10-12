Running
=======

Start the bridge
----------------

The bridge exposes a WebSocket server (default
``ws://127.0.0.1:3000/ws/bridge``) and connects to a data source selected with
``--protocol``. All commands are run from the ``bridge`` directory.

Simulator (no hardware required)::

   uv run bridge --protocol simulator --port 3000

Serial device (COBS framed CBOR)::

   uv run bridge --protocol serial --device /dev/ttyACM0 --baud 1000000

TCP / UDP endpoint::

   uv run bridge --protocol tcp --remote-host 127.0.0.1 --remote-port 9000

Common options:

.. list-table::
   :header-rows: 1
   :widths: 25 20 55

   * - Option
     - Default
     - Description
   * - ``--host``
     - ``127.0.0.1``
     - WebSocket server bind address
   * - ``--port``
     - ``3000``
     - WebSocket server port
   * - ``--path``
     - ``/ws/bridge``
     - WebSocket endpoint path
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
