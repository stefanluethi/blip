outpost
=======

The outpost is a Python library, the package ``blip-outpost``, to send the
traces, log records and actions of a target to blip. Your script talks to the
target in whatever way it needs -- a serial port, a socket, a CAN bus, a vendor
library -- decodes what it receives and passes it on with ``send_value``. The
outpost serves blip over a WebSocket, records the messages if asked, and calls
your **actions** when blip invokes them.

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
   outpost.send_metadata("room/temperature", "°C")
   while True:
       outpost.send_value("room/temperature", 21.0 + random.random())
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
       nested freely. Timestamped now (UTC) unless given; while the outpost
       passes a target through or translates it, with the time of the target
       instead, and dropped before the target sent its first timestamp.
   * - ``send_metadata(path, unit)``
     - the unit of a trace; for a record field, the path joined with ``.``,
       e.g. ``imu.acc``
   * - ``send_log(level, message, component="", timestamp_ns=None, path="")``
     - a log record of a ``LogLevel``, at ``path`` like a trace, e.g.
       ``motor``, or without one of the target as a whole
   * - ``send(message)``
     - a ``Message``, or the bytes of complete messages, e.g. as received
       from a target that speaks the blip protocol

They may be called from any thread and never block: the outpost passes the
messages on from its own thread. Python ints are sent as CBOR integers, so pass
a float for a float trace. Timestamps are nanoseconds, either UTC or the time
since the target booted; see :doc:`/overview/concepts`.

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
* :doc:`examples` -- the example scripts: the smallest, a simulator and a
  stress test.
* :doc:`/reference/outpost-api` -- all functions and classes of ``blip_outpost``.
* :doc:`/help/limits` -- before serving the outpost on a network.
* :doc:`/help/troubleshooting` -- when blip shows nothing.

.. toctree::
   :hidden:

   examples
   internals
