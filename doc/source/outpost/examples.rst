outpost examples
================

The ``outpost/examples`` folder of the repository holds scripts to try blip and
to copy from. They are not part of the library; the simulator needs ``psutil``,
which ``uv sync`` installs in the ``outpost`` directory::

   cd outpost
   uv sync
   uv run python examples/simulator.py

minimal
-------

``examples/minimal.py``: the smallest script, sending a sine wave with noise
and a log record now and then, with an action that sets the amplitude. It uses
the functions of the module from plain, blocking code:

.. literalinclude:: /../../outpost/examples/minimal.py
   :language: python
   :start-at: import math

app
---

``examples/app.py``: a Python program that traces itself, a room heater with a
controller. Each part has a tracer of its own; the controller sends its state
as a dataclass record with units, a fast sensor sends blocks of samples with
``series``, Python's ``logging`` reaches blip through a ``LogHandler``, and the
action ``room/setpoint`` sets the setpoint. See
:ref:`outpost-tracers`.

.. literalinclude:: /../../outpost/examples/app.py
   :language: python
   :start-at: # the program's own logging
   :end-before: def read_block

simulator
---------

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

It also sends log records at a few paths below ``log`` at all levels, every
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
----

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

Next steps
----------

* :doc:`index` -- write a script for your own target, starting from these.
* :doc:`/help/limits` -- how many messages an outpost and blip take.
