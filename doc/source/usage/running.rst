Running
=======

Start the outpost
-----------------

The outpost exposes a WebSocket server (default
``ws://127.0.0.1:3000/ws/outpost``) and talks to the target through the
protocol selected with ``--protocol``. All commands are run from the
``outpost`` directory.

Simulator (no hardware required)::

   uv run outpost --protocol simulator --port 3000

The simulator streams per-core CPU load and an accelerometer value, an ``imu``
record of two 3-element arrays, a ``motor`` record of an integer, a bool and a
string, and a temperature that is ``null`` now and then. It also emits log
records from a few synthetic components at all severity levels. Sustained
high CPU load on a core is reported as a warning.

Its actions change what it sends: ``motor/enable`` switches the motor on and
off, ``motor/speed`` sets its speed setpoint (rejected while the motor is
off), ``thermal/ambient`` the ambient temperature of the thermal frame,
``log/level`` the lowest level of the log records sent, ``log/message`` sends
a log record and ``imu/calibrate`` asks for confirmation and is disabled while
it runs.

The options of a protocol are set with ``--option key=value`` (``-o``),
converted by the type hints of its constructor, and listed by
``--protocol-help``::

   uv run outpost --protocol simulator --protocol-help
   uv run outpost --protocol simulator -o interval_ms=10 -o log_interval_ms=200

TCP or UDP endpoint of a target that speaks the blip protocol itself::

   uv run outpost --protocol tcp -o remote_host=127.0.0.1 -o remote_port=9000
   uv run outpost --protocol udp -o remote_host=127.0.0.1 -o remote_port=9000

Add ``-o listen=true`` to accept connections from the target instead of
connecting to it. The data of the target is passed to blip unchanged, the
invocations of blip are forwarded as ``Invoke`` requests, and the target
announces its actions itself. An invocation is accepted once it is forwarded;
``ActionResult`` messages of the target are dropped, as the outpost answers
blip itself.

Replay a recording, saved by blip or recorded by the outpost::

   uv run outpost --protocol replay -o path=recording.blip

The messages are sent unchanged, each when the time between its timestamp
and the first one has passed, divided by ``-o speed=…``. The replay starts
when the first client connects. The actions of the recording are shown
disabled, as a replay carries none out.

Record the messages sent to blip with any protocol::

   uv run outpost --protocol simulator --record simulator.blip

The file is written while the outpost runs and can be opened in blip with
**File > Open recording**.

Options:

.. list-table::
   :header-rows: 1
   :widths: 25 20 55

   * - Option
     - Default
     - Description
   * - ``--protocol`` / ``-p``
     - ``simulator``
     - ``simulator``, ``replay``, ``tcp``, ``udp``, or a Python file or
       module of a protocol, followed by ``:ClassName`` if it defines several
   * - ``--option`` / ``-o``
     -
     - An option of the protocol as ``key=value``, repeat for several
   * - ``--protocol-help``
     -
     - Show the options of the protocol and exit
   * - ``--host`` / ``-h``
     - ``127.0.0.1``
     - WebSocket server bind address
   * - ``--port``
     - ``3000``
     - WebSocket server port
   * - ``--path``
     - ``/ws/outpost``
     - WebSocket endpoint path
   * - ``--record``
     -
     - Record the messages sent to blip to this file
   * - ``--verbose`` / ``-v``
     - off
     - Enable debug output

Run ``uv run outpost --help`` for the complete list.

Write a protocol
----------------

A protocol connects the outpost to a target of your own. It is a Python class
using the public API of the outpost, kept in your project::

   from typing import Annotated, Literal

   from outpost import ActionError, Protocol, Range, Unit, action


   class MyTarget(Protocol):
       def __init__(self, host: str, port: int = 9000) -> None:
           self.host, self.port = host, port
           self.link = None

       async def run(self) -> None:
           ...  # connect, translate what the target sends, e.g.
           await self.send_value("motor/speed_rpm", 1432)

       @action("motor/speed", readback="motor/speed_rpm")
       async def set_speed(self, rpm: Annotated[int, Range(0, 3000), Unit("rpm")]) -> None:
           """Speed setpoint"""
           if self.link is None:
               raise ActionError("not connected")
           ...

       @action("log/level")
       async def set_log_level(self, level: Literal["debug", "info", "warn"] = "info") -> None:
           ...

Run it by naming its file, or its module if it is installed::

   uv run outpost --protocol ./my_target.py -o host=192.168.1.70

or serve it from a script of your own with
``outpost.run(MyTarget("192.168.1.70"))``.

``run`` talks to the target until the outpost cancels it at shutdown. It sends
to blip with ``send_value``, ``send_metadata``, ``send_log`` or ``send``.
Returning ends the protocol but not the outpost, an exception stops the
outpost. ``on_client_connected`` is called when blip connects, e.g. to send
units right away.

Methods decorated with ``@action`` are the actions blip shows. Their
parameters are taken from the type hints: ``bool``, ``int``, ``float``,
``str``, ``Literal`` of strings and ``enum.Enum`` (offered by member name),
optionally in ``Annotated`` with a ``Range`` and a ``Unit``. A default value
is the initial value in blip. ``@action`` also takes a ``description``
(default: the first line of the docstring), a ``readback`` trace path that
shows the current value of what the action sets, and ``confirm`` to ask the
user before sending.

The outpost checks the arguments, then carries out the invocations of an
action one after another, in the order blip sent them. An invocation is
accepted when the action returns and rejected when it raises,
``ActionError`` with the reason blip shows. Actions can be added, removed,
enabled and disabled at runtime through ``self.actions``, e.g.
``self.actions.set_enabled(False)`` while the link to the target is down;
blip gets the new list right away. A protocol for a target that knows the
actions itself overrides ``invoke`` to forward them.

The simulator (``outpost/src/outpost/protocols/simulator.py``) only uses the
public API and serves as an example.

Start blip
----------

Web (development server with live reload)::

   cd blip && trunk serve

then open http://127.0.0.1:8080 in a browser.

Native::

   cargo run --release -p blip
