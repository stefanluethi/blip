outpost
=======

The outpost is a Python library, the package ``blip-outpost``, to send the
traces, log records and actions of a target to blip. Your script talks to the
target in whatever way it needs -- a serial port, a socket, a CAN bus, a vendor
library -- decodes what it receives and passes it on with ``value`` and ``log``.
A Python program that traces itself gets tracers by name, as it gets loggers.
The outpost serves blip over a WebSocket, records the messages if asked, and
calls your **actions** when blip invokes them.

.. admonition:: Before you begin

   The outpost installed in your project, Python 3.10 or newer (see
   :doc:`/start/installation`), and a way to talk to your target from Python, e.g.
   pyserial for a serial port.

.. figure:: /diagrams/setups.svg
   :target: ../_images/setups.svg
   :class: diagram
   :alt: The simulator example, your own script, the TCP or UDP passthrough
         for targets with sonde, and the replay of recordings all serve blip
         over a WebSocket.

   What the outpost is used for.

Getting started
---------------

Add the library to your project, see :doc:`/start/installation`::

   pip install "blip-outpost @ git+https://github.com/stefanluethi/blip#subdirectory=outpost"

Start the outpost, then send from your own loop:

.. code-block:: python

   import random
   import time

   import blip_outpost as outpost

   outpost.start()
   outpost.unit("room/temperature", "°C")
   while True:
       outpost.value("room/temperature", 21.0 + random.random())
       time.sleep(0.1)

``start`` serves ``ws://127.0.0.1:3000/ws/outpost`` from a thread of its own
and returns; the script carries on with its own work. Connect blip to that
address, see :doc:`/blip/connecting`. Several blips can be connected at once;
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
       outpost.unit("motor/speed_rpm", "rpm")
       while True:
           line = link.readline().decode().strip()   # e.g. "speed=1432 fault=0"
           if not line:
               continue
           fields = dict(item.split("=") for item in line.split())
           outpost.value("motor/speed_rpm", int(fields["speed"]))
           if fields["fault"] != "0":
               outpost.log(LogLevel.ERROR, f"fault {fields['fault']}", path="log/motor")


   main()

Run it as any Python script, ``python my_target.py``. It stops with
:kbd:`Ctrl+C`; the recording is finished when the script exits.

Sending to blip
~~~~~~~~~~~~~~~

Each call carries all of a message: the full path, and for a log record the
level and the path, as the target sent them. Nothing needs to be
declared first.

.. list-table::
   :header-rows: 1
   :widths: 45 55

   * - Function
     - Sends
   * - ``value(path, value, timestamp_ns=None)``
     - a sample; ``value`` is ``None``, ``int``, ``float``, ``bool``,
       ``str``, a ``list`` of fixed length or a record: a ``dict`` with
       ``str`` keys or a dataclass, nested freely. Timestamped now (UTC)
       unless given; while the outpost passes a target through or translates
       it, with the time of the target instead, and dropped before the target
       sent its first timestamp.
   * - ``series(path, values, timestamps_ns=…)``,
       ``series(path, values, start_ns=…, period_ns=…)``
     - many samples of one trace at once, e.g. a block read from an ADC: at
       their timestamps, or the first at ``start_ns`` and then one every
       ``period_ns``
   * - ``unit(path, unit)``
     - the unit of a trace; for a record field, the path joined with ``.``,
       e.g. ``imu.acc``
   * - ``log(level, message, path="", timestamp_ns=None)``
     - a log record of a ``LogLevel`` or its number, at ``path`` like a trace,
       by convention ``log/<part>``, e.g. ``log/motor``, or without one:
       blip shows it at ``log/default``
   * - ``send(message)``
     - a ``Message``, or the bytes of complete messages, e.g. as received
       from a target that speaks the blip protocol

They may be called from any thread and never block: the outpost passes the
messages on from its own thread. Python ints are sent as CBOR integers, so pass
a float for a float trace. Timestamps are nanoseconds, either UTC or the time
since the target booted; see :doc:`/overview/concepts`. What has a ``tolist``
method, e.g. a numpy array or scalar, is sent as what it returns.

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

Records and batches
~~~~~~~~~~~~~~~~~~~

Values that belong together, e.g. the fields of one frame of the target, go in
a record: one message, one timestamp, and a trace per field, its path joined
with ``.``. blip shows records as rows of a table, see :doc:`/blip/views/table`.

.. code-block:: python

   outpost.value("motor", {"speed": 1432, "current": 2.1})   # motor.speed, motor.current

Separate traces that should be sent together go in a ``batch``: what the
thread sends in the block is handed over at its end, at once. What is sent
without a timestamp in the block shares one, ``timestamp_ns`` or the time it is
first needed:

.. code-block:: python

   for frame in link.frames():
       with outpost.batch(timestamp_ns=frame.t_us * 1000):
           outpost.value("motor", {"speed": frame.speed, "current": frame.current})
           outpost.value("battery/voltage", frame.vbat)
           for record in frame.logs:
               outpost.log(record.level, record.text, path=f"log/{record.module}")

Batches nest, the outermost one hands over. What was sent is handed over also
when the block raises. A batch belongs to its thread: in asyncio code, don't
``await`` inside the block, or what other tasks send meanwhile joins it.

.. _outpost-actions:

Actions
~~~~~~~

Functions decorated with ``@action`` are the actions blip shows (see
:doc:`/blip/actions`). Their parameters are taken from the type hints:

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
reason blip shows, any other exception with its type and message. Only the
blip that invoked the action gets the answer; the outpost sends no log records
about invocations.

Actions can be added, removed, enabled and disabled at any time through the
``actions`` of the outpost; blip gets the new list right away:

.. code-block:: python

   actions = outpost.get_outpost().actions
   actions.set_enabled(False)                    # all, e.g. while the link is down
   actions.set_enabled(True, "motor/speed")      # some
   actions.set_enabled(False, "imu/calibrate", reason="calibrating")  # shown in blip
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
   left.value("wheel/speed", 1.5)

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
               outpost.value("motor/speed_rpm", int(line))


   asyncio.run(main())

In the event loop, ``await outpost.drain_async()`` and ``await
outpost.wait_for_client_async()`` take the place of ``drain`` and
``wait_for_client``. ``sleep_until(deadline)`` sleeps until a
``time.perf_counter()`` deadline, also on Windows, whose timers fire up to 16 ms
early; for sampling at a steady rate. The simulator example is such a script.

.. _outpost-tracers:

Tracing a Python program
------------------------

A program that traces itself, e.g. a service on a Linux machine, gets
**tracers** by name, as it gets loggers. The name is a path; the traces of a
tracer are below it, and its log records are at it. The
same name gives the same tracer anywhere in the program, so each module gets
its own when it is imported, also before the outpost serves:

.. code-block:: python

   import blip_outpost as outpost

   motor = outpost.get_tracer("motor")
   left = motor.child("left")                     # motor/left

   motor.unit("speed", "rpm")
   motor.value("speed", 1432)                     # the trace motor/speed
   motor.info("homed")                            # a log record at motor
   motor.warn("hot: %d °C", temperature)          # formatted as logging does
   left.series("current", block, start_ns=t0, period_ns=50_000)

   with motor.batch():                            # one timestamp for all of it
       motor.value("speed", speed)
       motor.value("current", current)

A tracer has ``value``, ``series``, ``unit`` and ``batch`` as above, with paths
below its name, and ``log(level, message, *args)`` with ``debug``, ``info``,
``warn`` (or ``warning``) and ``error``. ``@motor.action("speed",
readback="speed_rpm")`` makes the action ``motor/speed`` with the readback
``motor/speed_rpm``. ``outpost.get_tracer()`` without a name is of the target
as a whole.

A dataclass is a record, its fields the fields of the record. The units of its
fields are declared with ``Unit`` and sent with its first sample at a path:

.. code-block:: python

   from dataclasses import dataclass
   from typing import Annotated

   from blip_outpost import Unit


   @dataclass
   class HeaterState:
       power: Annotated[float, Unit("W")]
       duty: Annotated[float, Unit("%")]
       on: bool


   heater.value("state", HeaterState(power=750.0, duty=50.0, on=True))
   # heater/state.power in W, heater/state.duty in %, heater/state.on

The records of Python's ``logging`` reach blip with a ``LogHandler``, the name
of the logger below a prefix, ``log`` unless ``path`` is given: the logger
``app.motor`` sends to ``log/app/motor``. The root logger sends with an empty
path, shown at ``log/default``, or at ``path`` if given:

.. code-block:: python

   import logging

   logging.getLogger().addHandler(outpost.LogHandler())

The records of the outpost itself and of the libraries it uses are not sent.
The example ``app.py`` is such a program, see :doc:`examples`.

Recording
---------

Pass ``record`` to ``start`` (or ``--record`` on the command line)::

   outpost.start(record="run.blip")

The file holds all messages sent to blip, also while no blip is connected,
starting with the units and actions sent before. It is flushed every second, so
it can be read up to about a second before the script stopped, also if it was
killed. Open it in blip with **File › Open recording** or replay it. The
format is described in :doc:`/reference/protocol`.

Serving without a script
------------------------

Targets that speak the blip protocol themselves, e.g. with :doc:`/sonde/index`, targets
that send protobuf messages of an annotated schema, and recordings need no
script: the ``blip-outpost`` command serves them::

   blip-outpost tcp 192.168.1.70 9000
   blip-outpost udp 0.0.0.0 9000 --listen
   blip-outpost protobuf ws://192.168.1.70/ws/robby --proto link.proto --uplink my.link.Uplink
   blip-outpost replay recording.blip --speed 2

The commands, their options and the annotations of protobuf schemas are listed
in :doc:`/reference/outpost-cli`.

Next steps
----------

* :doc:`/blip/connecting` -- connect blip to the outpost.
* :doc:`examples` -- the example scripts: the smallest, a program that traces
  itself, a simulator and a stress test.
* :doc:`/reference/outpost-api` -- all functions and classes of ``blip_outpost``.
* :doc:`/help/limits` -- before serving the outpost on a network.
* :doc:`/help/troubleshooting` -- when blip shows nothing.

.. toctree::
   :hidden:

   examples
   internals
