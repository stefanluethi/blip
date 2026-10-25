outpost
=======

The outpost is a small asynchronous Python service next to a target. It talks
to the target with a **protocol** -- a Python class, built in or your own --
and serves blip over a WebSocket. It also records the messages it sends, and
carries out the **actions** blip invokes.

.. figure:: /diagrams/setups.svg
   :target: ../_images/setups.svg
   :class: diagram
   :alt: The simulator, your own protocol, the TCP or UDP passthrough for
         targets with sonde, and the replay of recordings all serve blip over
         a WebSocket.

   What the outpost is used for.

Running it
----------

From the ``outpost`` directory (see :doc:`installation`)::

   uv run outpost --protocol simulator

It serves ``ws://127.0.0.1:3000/ws/outpost`` until stopped with
:kbd:`Ctrl+C`. Connect blip to that address, see :doc:`blip/connecting`.
Several blips can be connected at once; each gets all messages.

.. list-table:: Command line
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
     - Address the WebSocket server binds to; ``0.0.0.0`` serves blips on
       other machines too
   * - ``--port``
     - ``3000``
     - WebSocket server port
   * - ``--path``
     - ``/ws/outpost``
     - WebSocket endpoint path; connections to another path are rejected
       (the root path ``/`` is always accepted)
   * - ``--record``
     -
     - Record the messages sent to blip to this file
   * - ``--verbose`` / ``-v``
     - off
     - Debug output

The options of a protocol are the keyword arguments of its constructor,
converted by their type hints (``bool`` takes ``true``/``false``, ``yes``/``no``,
``on``/``off`` or ``1``/``0``; ``int`` also takes ``0x…``)::

   uv run outpost --protocol simulator --protocol-help
   uv run outpost --protocol simulator -o interval_ms=10 -o log_interval_ms=200

The outpost stops with an error if the protocol raises. If the protocol ends,
e.g. a replay at the end of its file, the outpost keeps serving until it is
stopped, so blip still sees what was sent.

Built-in protocols
------------------

simulator
~~~~~~~~~

A made-up target to try blip without hardware; also the reference for writing
protocols (``outpost/src/outpost/protocols/simulator.py``). It sends:

.. list-table::
   :header-rows: 1
   :widths: 30 25 45

   * - Path
     - Kind
     - Every
   * - ``accelerometer``
     - float, m/s²
     - sampling interval (``interval_ms``, 20 ms)
   * - ``imu`` (``.acc``, ``.gyro``)
     - record of two ``float[3]``
     - sampling interval
   * - ``audio/spectrum``
     - ``float[64]``, dB
     - 33 ms
   * - ``motor`` (``.speed_rpm``, ``.enabled``, ``.state``)
     - record of int, bool, string
     - 50 ms
   * - ``robot/pose`` (``.x``, ``.y``, ``.heading``)
     - record of floats, a figure eight
     - 100 ms
   * - ``thermal/frame``
     - ``float[8×8]``, °C
     - 115 ms
   * - ``sysinfo/cpu/cpuN``
     - float, %, the load of each core of the machine
     - 250 ms
   * - ``sensor/temperature``
     - float, °C, ``null`` now and then
     - 1 s
   * - ``simulator/log_level``
     - string
     - on change

It also sends log records of a few components at all levels, every
``log_interval_ms`` (500 ms) on average, and a warning when the load of a core
stays high. Its actions:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Action
     - Does
   * - ``motor/enable``
     - switches the motor on and off (readback ``motor.enabled``)
   * - ``motor/speed``
     - sets the speed setpoint, 0 to 3000 rpm (readback ``motor.speed_rpm``);
       rejected while the motor is off
   * - ``thermal/ambient``
     - the ambient temperature of the thermal frame
   * - ``log/level``
     - the lowest level of the log records sent (readback
       ``simulator/log_level``)
   * - ``log/message``
     - sends a log record of a level and text
   * - ``imu/calibrate``
     - asks for confirmation, takes two seconds and is disabled meanwhile

tcp and udp
~~~~~~~~~~~

For targets that speak the blip protocol themselves, e.g. firmware using
:doc:`sonde`::

   uv run outpost --protocol tcp -o remote_host=192.168.1.70 -o remote_port=9000
   uv run outpost --protocol udp -o remote_host=0.0.0.0 -o remote_port=9000 -o listen=true

.. list-table::
   :header-rows: 1
   :widths: 25 20 55

   * - Option
     - Default
     - Description
   * - ``remote_host``
     - ``127.0.0.1``
     - the target's address, or the address to listen on
   * - ``remote_port``
     - ``9000``
     - the target's port, or the port to listen on
   * - ``listen``
     - ``false``
     - accept connections (TCP) or datagrams (UDP) from the target instead of
       connecting to it

* The target sends its messages as CBOR, any number at once as a CBOR
  sequence: each UDP datagram holds complete messages, over TCP they may be
  split anywhere. The outpost forwards each complete message to blip
  unchanged and drops data that is no CBOR. As a TCP client it reconnects
  every three seconds after an error.
* Invocations from blip are forwarded to the target as ``Invoke`` requests; an
  invocation counts as accepted once it is forwarded. The target announces its
  actions itself; ``ActionResult`` messages of the target are dropped, as the
  outpost answers blip itself.

replay
~~~~~~

Replays a recording, saved by blip or written with ``--record`` or by sonde::

   uv run outpost --protocol replay -o path=recording.blip -o speed=2

The messages are sent unchanged, each when the time between its timestamp and
the first one has passed, divided by ``speed``. The replay starts when the
first blip connects. The actions of the recording are shown disabled, as a
replay carries none out.

Recording
---------

Any protocol can be recorded::

   uv run outpost --protocol simulator --record simulator.blip

The file holds all messages sent to blip, also while no blip is connected, and
is flushed every second, so it can be read up to about a second before the
outpost stopped, also if it was killed. Open it in blip with **File › Open
recording** or replay it. The format is described in
:doc:`/architecture/protocol`.

Writing a protocol
------------------

A protocol connects the outpost to a target of your own. It is a Python class
using the public API of the outpost, kept in your project:

.. code-block:: python

   from typing import Annotated, Literal

   from outpost import ActionError, LogLevel, Protocol, Range, Unit, action


   class MyTarget(Protocol):
       """Talks to my target over TCP."""

       def __init__(self, host: str, port: int = 9000) -> None:
           # the options, set with -o host=… -o port=…
           self.host, self.port = host, port
           self.link = None

       async def run(self) -> None:
           # connect, then translate what the target sends
           self.link = await connect(self.host, self.port)
           await self.send_metadata("motor/speed_rpm", "rpm")
           while True:
               frame = await self.link.read()
               await self.send_value("motor/speed_rpm", frame.speed)
               if frame.fault:
                   await self.send_log(LogLevel.ERROR, f"fault {frame.fault}", component="motor")

       async def on_client_connected(self) -> None:
           # blips connecting later get the units too
           await self.send_metadata("motor/speed_rpm", "rpm")

       @action("motor/speed", readback="motor/speed_rpm")
       async def set_speed(self, rpm: Annotated[int, Range(0, 3000), Unit("rpm")] = 1500) -> None:
           """Speed setpoint"""
           if self.link is None:
               raise ActionError("not connected")
           await self.link.write_setpoint(rpm)

Run it by naming its file, followed by ``:ClassName`` if it defines several
protocols, or its module if it is installed::

   uv run outpost --protocol ./my_target.py -o host=192.168.1.70

or serve it from a script of your own:

.. code-block:: python

   import outpost

   outpost.run(MyTarget("192.168.1.70"), port=3000, record="run.blip")

Lifecycle
~~~~~~~~~

* The outpost creates the protocol, announces its actions, starts ``run`` and
  then the WebSocket server.
* ``run`` talks to the target until the outpost cancels it at shutdown.
  Returning ends the protocol but not the outpost; an exception stops the
  outpost.
* ``on_client_connected`` is called whenever a blip connects, after the
  actions were sent to it, e.g. to send units right away: the outpost passes on
  messages as they are sent and keeps only the latest list of actions for blips
  connecting later.

Sending to blip
~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 45 55

   * - Method
     - Sends
   * - ``send_value(path, value, timestamp_ns=None)``
     - a sample; ``value`` is ``None``, ``int``, ``float``, ``bool``,
       ``str``, a ``list`` of fixed length or a ``dict`` with ``str`` keys,
       nested freely. Timestamped now (UTC) unless given.
   * - ``send_metadata(path, unit)``
     - the unit of a trace; for a record field, the path joined with ``.``,
       e.g. ``imu.acc``
   * - ``send_log(level, message, component="", timestamp_ns=None)``
     - a log record of a ``LogLevel``
   * - ``send(message)``
     - a ``Message``, or the bytes of complete messages, e.g. as received
       from a target that speaks the blip protocol

Python ints are sent as CBOR integers, so pass a float for a float trace.
Timestamps are nanoseconds, either UTC or the time since the target booted;
see :doc:`concepts`.

A slow blip does not hold up the protocol: each blip has its own queue of
messages, and messages are dropped (with a warning) when it holds more than
10 000.

.. _outpost-actions:

Actions
~~~~~~~

Methods decorated with ``@action`` are the actions blip shows (see
:doc:`blip/actions`). Their parameters are taken from the type hints:

.. list-table::
   :header-rows: 1
   :widths: 40 60

   * - Type hint
     - Input in blip
   * - ``bool``
     - checkbox
   * - ``int``, ``float``
     - slider with a ``Range(min, max)``, else a number field
   * - ``Literal["a", "b"]``, an ``enum.Enum``
     - buttons or a dropdown (enums by member name)
   * - ``str``
     - text field
   * - none
     - an action without parameters is a **Run** button

Wrap a type in ``Annotated`` with a ``Range(min, max, step)`` and a
``Unit("rpm")``. A default value is the initial value in blip; a parameter
without a default must be given.

``@action(path, description="", readback=None, confirm=False)`` also takes a
``description`` (by default the first line of the docstring), a ``readback``
trace path that shows the current value of what the action sets, and
``confirm`` to ask the user before sending.

The outpost checks the arguments against the parameters, then carries out the
invocations of an action one after another, in the order blip sent them;
different actions run concurrently. An invocation is accepted when the action
returns and rejected when it raises: ``ActionError`` with the reason blip
shows, any other exception with its type and message. The outpost logs each
invocation to all blips with the component ``actions``.

Actions can be added, removed, enabled and disabled at runtime through
``self.actions``; blip gets the new list right away:

.. code-block:: python

   self.actions.set_enabled(False)                    # all, e.g. while the link is down
   self.actions.set_enabled(True, "motor/speed")      # some
   self.actions.add("pump/run", self.run_pump, description="Runs the pump")
   self.actions.remove("pump/run")

A protocol for a target that knows its actions itself overrides
``invoke(path, args)`` to forward them, as the ``tcp`` and ``udp`` protocols
do.

API summary
-----------

Everything a protocol needs is exported by the ``outpost`` package:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Name
     - Purpose
   * - ``Protocol``
     - base class: ``run``, ``on_client_connected``, ``invoke``, ``send_*``,
       ``announce_actions``, ``actions``
   * - ``action``
     - decorator marking a method as an action
   * - ``Actions``
     - the registry behind ``Protocol.actions``: ``add``, ``remove``,
       ``clear``, ``set_enabled``, ``is_enabled``, ``invoke``
   * - ``Range``, ``Unit``
     - ``Annotated`` metadata of action parameters
   * - ``ActionError``
     - raised by an action to reject an invocation
   * - ``LogLevel``
     - ``TRACE``, ``DEBUG``, ``INFO``, ``WARN``, ``ERROR``
   * - ``Message``, ``Data``
     - building messages by hand: ``Message.create_value``,
       ``create_metadata``, ``create_log``, ``create_actions``
   * - ``run``
     - serve a protocol from a script: ``run(protocol, host=…, port=…,
       path=…, record=…)``

Tests
-----

::

   cd outpost && uv run pytest
