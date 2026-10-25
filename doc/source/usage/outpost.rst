outpost
=======

The outpost is a Python library, the package ``blip-outpost``, to send the
traces, log records and actions of a target to blip. Your script talks to the
target in whatever way it needs -- a serial port, a socket, a CAN bus, a vendor
library -- decodes what it receives and passes it on with ``send_value``. The
outpost serves blip over a WebSocket, records the messages if asked, and calls
your **actions** when blip invokes them.

.. figure:: /diagrams/setups.svg
   :target: ../_images/setups.svg
   :class: diagram
   :alt: The simulator example, your own script, the TCP or UDP passthrough
         for targets with sonde, and the replay of recordings all serve blip
         over a WebSocket.

   What the outpost is used for.

Getting started
---------------

Add the library to your project, see :doc:`installation`::

   pip install "blip-outpost @ git+https://github.com/stefanluethi/blip#subdirectory=outpost"

Start the outpost, then send from your own loop:

.. code-block:: python

   import random
   import time

   import blip_outpost as outpost

   outpost.start()
   outpost.send_metadata("room/temperature", "°C")
   while True:
       outpost.send_value("room/temperature", 21.0 + random.random())
       time.sleep(0.1)

``start`` serves ``ws://127.0.0.1:3000/ws/outpost`` from a thread of its own
and returns; the script carries on with its own work. Connect blip to that
address, see :doc:`blip/connecting`. Several blips can be connected at once;
each gets all messages.

Writing a script
----------------

A script for a real target reads from the target, decodes what it gets and
passes it on. Its actions are functions that write to the target:

.. code-block:: python

   from typing import Annotated

   import serial  # pyserial, or whatever talks to your target

   import blip_outpost as outpost
   from blip_outpost import ActionError, LogLevel, Range, Unit

   link = serial.Serial("COM3", 115200, timeout=1)


   @outpost.action("motor/speed", readback="motor/speed_rpm")
   def set_speed(rpm: Annotated[int, Range(0, 3000), Unit("rpm")] = 1500) -> None:
       """Speed setpoint"""
       if not link.is_open:
           raise ActionError("the serial port is closed")
       # the effect shows in the readback, motor/speed_rpm
       link.write(f"SPEED {rpm}\n".encode())


   def main() -> None:
       outpost.start(record="run.blip")
       outpost.send_metadata("motor/speed_rpm", "rpm")
       while True:
           line = link.readline().decode().strip()   # e.g. "speed=1432 fault=0"
           if not line:
               continue
           fields = dict(item.split("=") for item in line.split())
           outpost.send_value("motor/speed_rpm", int(fields["speed"]))
           if fields["fault"] != "0":
               outpost.send_log(LogLevel.ERROR, f"fault {fields['fault']}", component="motor")


   main()

Run it as any Python script, ``python my_target.py``. It stops with
:kbd:`Ctrl+C`; the recording is finished when the script exits.

Sending to blip
~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 45 55

   * - Function
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

They may be called from any thread and never block: the outpost passes the
messages on from its own thread. Python ints are sent as CBOR integers, so pass
a float for a float trace. Timestamps are nanoseconds, either UTC or the time
since the target booted; see :doc:`concepts`.

The units and the actions are kept and sent to each blip when it connects, so
a script sends them once, also before ``start``. Other messages reach the blips
connected at the time; before ``start`` and after ``stop`` they are dropped.

A slow blip does not hold up the script: each blip has its own queue of
messages, and messages are dropped (with a warning) when it holds more than
10 000. A script that sends faster than blip takes them, e.g. data of the past,
calls ``drain()`` every thousand messages or so: it blocks until every blip's
queue is at most half full, so the script slows down to the slowest blip
instead of losing messages. ``wait_for_client(timeout=None)`` blocks until a
blip is connected, e.g. to send a history only then.

.. _outpost-actions:

Actions
~~~~~~~

Functions decorated with ``@action`` are the actions blip shows (see
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

A function becomes an action as it is defined. A method of a class becomes an
action of each object passed to ``register``, so the actions share the state
of the object, e.g. the link to the target:

.. code-block:: python

   class Motor:
       def __init__(self, link) -> None:
           self.link = link

       @outpost.action("motor/stop", confirm=True)
       def stop(self) -> None:
           self.link.write(b"STOP\n")


   outpost.register(Motor(link))

The outpost checks the arguments against the parameters, then carries out the
invocations of an action one after another, in the order blip sent them;
different actions run concurrently. A plain function runs in a thread of its
own, so it may block, e.g. waiting for the target to answer; an ``async``
function runs in the event loop of the outpost. An invocation is accepted when
the action returns and rejected when it raises: ``ActionError`` with the
reason blip shows, any other exception with its type and message. The outpost
logs each invocation to all blips with the component ``actions``.

Actions can be added, removed, enabled and disabled at any time through the
``actions`` of the outpost; blip gets the new list right away:

.. code-block:: python

   actions = outpost.get_outpost().actions
   actions.set_enabled(False)                    # all, e.g. while the link is down
   actions.set_enabled(True, "motor/speed")      # some
   actions.add("pump/run", run_pump, description="Runs the pump")
   actions.remove("pump/run")

Serving
~~~~~~~

``start(host="127.0.0.1", port=3000, path="/ws/outpost", record=None)`` serves
from a thread of its own until ``stop()`` or the end of the script.
``host="0.0.0.0"`` serves blips on other machines too; ``record`` writes all
messages to a recording file, see `Recording`_. A script that only offers
actions calls ``wait()`` after ``start``, which blocks until :kbd:`Ctrl+C`.

The functions of the module use one outpost of the script. For several, e.g.
on different ports, create an ``Outpost`` each; it has the same methods:

.. code-block:: python

   from blip_outpost import Outpost

   left = Outpost(port=3001).start()
   right = Outpost(port=3002).start()
   left.send_value("wheel/speed", 1.5)

   @right.action("wheel/brake")
   def brake() -> None: ...

``with Outpost(...) as outpost:`` starts and stops it around a block.

asyncio
~~~~~~~

In asyncio code the outpost serves from the running event loop instead of a
thread of its own; its async actions run in that loop too, next to the code
that talks to the target:

.. code-block:: python

   import asyncio

   from blip_outpost import Outpost


   async def main() -> None:
       async with Outpost(port=3000) as outpost:
           reader, writer = await asyncio.open_connection("192.168.1.70", 9000)

           @outpost.action("motor/stop")
           async def stop() -> None:
               writer.write(b"STOP\n")
               await writer.drain()

           while line := await reader.readline():
               outpost.send_value("motor/speed_rpm", int(line))


   asyncio.run(main())

In the event loop, ``await outpost.drain_async()`` and ``await
outpost.wait_for_client_async()`` take the place of ``drain`` and
``wait_for_client``. ``sleep_until(deadline)`` sleeps until a
``time.perf_counter()`` deadline, also on Windows, whose timers fire up to 16 ms
early; for sampling at a steady rate. The simulator example is such a script.

Recording
---------

Pass ``record`` to ``start`` (or ``--record`` on the command line)::

   outpost.start(record="run.blip")

The file holds all messages sent to blip, also while no blip is connected,
starting with the units and actions sent before. It is flushed every second, so
it can be read up to about a second before the script stopped, also if it was
killed. Open it in blip with **File › Open recording** or replay it. The
format is described in :doc:`/architecture/protocol`.

Command line
------------

The ``blip-outpost`` command serves targets that need no script of their own:
those that speak the blip protocol themselves, and recordings. Each command
takes the options of the WebSocket server blip connects to:

.. list-table:: Options of every command
   :header-rows: 1
   :widths: 25 20 55

   * - Option
     - Default
     - Description
   * - ``--ws-host``
     - ``127.0.0.1``
     - Address the WebSocket server binds to; ``0.0.0.0`` serves blips on
       other machines too
   * - ``--ws-port``
     - ``3000``
     - WebSocket server port
   * - ``--ws-path``
     - ``/ws/outpost``
     - WebSocket endpoint path; connections to another path are rejected
       (the root path ``/`` is always accepted)
   * - ``--record``
     -
     - Record the messages sent to blip to this file
   * - ``--verbose`` / ``-v``
     - off
     - Debug output

tcp and udp
~~~~~~~~~~~

For targets that speak the blip protocol themselves, e.g. firmware using
:doc:`sonde`::

   blip-outpost tcp 192.168.1.70 9000
   blip-outpost udp 0.0.0.0 9000 --listen

The target is at ``HOST PORT``; with ``--listen`` the outpost accepts
connections (TCP) or datagrams (UDP) from the target there instead of
connecting to it.

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

Replays a recording, saved by blip or written with ``record`` or by sonde::

   blip-outpost replay recording.blip --speed 2

The messages are sent unchanged, each when the time between its timestamp and
the first one has passed, divided by ``--speed``. The replay starts when the
first blip connects, and the outpost serves until stopped after it ended. The
actions of the recording are shown disabled, as a replay carries none out.

Examples
--------

The ``outpost/examples`` folder of the repository holds scripts to try blip and
to copy from. They are not part of the library; the simulator needs ``psutil``,
which ``uv sync`` installs in the ``outpost`` directory::

   cd outpost
   uv sync
   uv run python examples/simulator.py

minimal
~~~~~~~

``examples/minimal.py``: the smallest script, sending a sine wave with noise
and a log record now and then, with an action that sets the amplitude. It uses
the functions of the module from plain, blocking code.

simulator
~~~~~~~~~

``examples/simulator.py``: a made-up target to try blip without hardware; also
the reference for scripts with asyncio and for actions as methods
(``--interval-ms``, ``--log-interval-ms``, ``--port``, ``--record``). It sends:

.. list-table::
   :header-rows: 1
   :widths: 30 25 45

   * - Path
     - Kind
     - Every
   * - ``accelerometer``
     - float, m/s²
     - sampling interval (``--interval-ms``, 20 ms)
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
``--log-interval-ms`` (500 ms) on average, and a warning when the load of a
core stays high. Its actions:

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

fast
~~~~

``examples/fast.py``: a stress test of blip, many traces at high rates, large
arrays and a flood of log records, to see how fast blip takes in data, how it
renders a lot of it, and how it copes with much data over time::

   uv run python examples/fast.py
   uv run python examples/fast.py --traces 500 --rate-hz 200
   uv run python examples/fast.py --history-s 600

.. list-table::
   :header-rows: 1
   :widths: 25 15 60

   * - Option
     - Default
     - Description
   * - ``--traces``
     - ``100``
     - float traces ``fast/signal/N``: sine, square, sawtooth and random walk
       shapes in turn, at frequencies from 0.05 Hz up to a twentieth of the
       sample rate, with noise
   * - ``--rate-hz``
     - ``1000``
     - samples per second of each of those traces
   * - ``--array-size``
     - ``1024``
     - elements of the array ``fast/spectrum`` (dB), a peak sweeping across
       it; ``0`` for none
   * - ``--frame-size``
     - ``64``
     - rows and columns of the frame ``fast/frame`` (°C), for the heatmap, a
       hot spot circling in it; ``0`` for none
   * - ``--array-rate-hz``
     - ``30``
     - samples per second of the array and the frame
   * - ``--log-rate-hz``
     - ``1000``
     - log records per second, of all levels, every hundredth a long one
   * - ``--history-s``
     - ``0``
     - seconds of data of the past, sent first as fast as blip takes it, once
       the action ``fast/start`` is run

The defaults are about 100 000 messages, 7.7 MB, per second. It starts when
the first blip connects. With ``--history-s`` it waits for the action
``fast/start`` instead, as blip keeps only what it receives while recording:
connect blip, start recording, then run ``fast/start`` in an actions view.
Unlike the other examples it does not drop messages for a blip that falls
behind, but waits for the slowest blip, so it measures how fast blip takes in
data. Every second it logs what it sent and how far behind real time it is,
and sends that as the traces ``fast/stats/messages_per_s``,
``fast/stats/megabytes_per_s`` and ``fast/stats/lag_s``. A lag that keeps
growing means blip does not keep up. With ``--history-s``, it logs how long the
history took once it caught up.

An outpost sends at most about 200 000 messages per second, as far as Python
gets; connect blip to several outposts on different ports for more.

API summary
-----------

Everything a script needs is exported by the ``blip_outpost`` package:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Name
     - Purpose
   * - ``start``, ``stop``, ``wait``
     - serve blip from a thread of its own, stop, block until :kbd:`Ctrl+C`
   * - ``send_value``, ``send_metadata``, ``send_log``, ``send``
     - send to blip, from any thread
   * - ``action``, ``register``
     - make a function an action, add the action methods of an object
   * - ``drain``, ``wait_for_client``
     - wait for room at the blips, wait for a blip
   * - ``get_outpost``
     - the ``Outpost`` the functions above use, e.g. for its ``actions``
   * - ``serve``
     - that outpost configured for ``async with``
   * - ``Outpost``
     - an outpost of its own: the methods above, ``async with``,
       ``drain_async``, ``wait_for_client_async``, ``actions``, ``url``,
       ``clients``, and ``on_invoke`` to carry out invocations in place of
       the actions, e.g. by forwarding them
   * - ``Actions``
     - the registry of actions: ``add``, ``remove``, ``clear``,
       ``set_enabled``, ``is_enabled``, ``batch``, ``invoke``
   * - ``Range``, ``Unit``
     - ``Annotated`` metadata of action parameters
   * - ``ActionError``
     - raised by an action to reject an invocation
   * - ``LogLevel``
     - ``TRACE``, ``DEBUG``, ``INFO``, ``WARN``, ``ERROR``
   * - ``Message``, ``Data``
     - building messages by hand: ``Message.create_value``,
       ``create_metadata``, ``create_log``, ``create_actions``
   * - ``sleep_until``
     - sleep until a ``time.perf_counter()`` deadline in asyncio code

Tests
-----

::

   cd outpost && uv run pytest
