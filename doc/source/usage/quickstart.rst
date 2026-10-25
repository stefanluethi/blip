Quick start
===========

This tour runs the outpost's simulator, a target made up in software, and
shows it in blip. It takes a few minutes and needs no hardware. Install the
outpost first, see :doc:`installation`.

.. figure:: /screenshots/window.png
   :target: ../_images/window.png
   :class: screenshot
   :alt: blip connected to an outpost, recording: plots of the acceleration and the motor speed, the states of the motor, the log, the traces tree, the layout and the properties of the selected plot, and the timeline.

   Where the tour ends up, give or take the layout.

.. tip::

   ``blip/tests/data`` has a 30 s recording of the simulator, ``demo.blip``, and a
   layout for it, ``demo-layout.json``. Drop both onto blip to look around
   without running anything.

.. figure:: /diagrams/setups.svg
   :target: ../_images/setups.svg
   :class: diagram
   :alt: Five ways to feed blip: the simulator, your target through your own
         protocol, a target with sonde through the TCP or UDP passthrough, a
         recording written by a unit test, and a replayed recording.

   Ways to feed blip. The tour uses the first one; the others work the same
   in blip.

1. Start the simulator
----------------------

::

   cd outpost
   uv run outpost --protocol simulator

It serves ``ws://127.0.0.1:3000/ws/outpost`` and logs each blip that connects.
Leave it running.

2. Start blip
-------------

Open the hosted web app at https://stefanluethi.github.io/blip/, or run blip
yourself, natively::

   cargo run --release -p blip

or in the browser (then open http://127.0.0.1:8080)::

   cd blip && trunk serve

blip starts with a tab holding a plot and a log view. The window has five
parts, see :doc:`blip/index`: the menu on top, the traces and the layout on
the left, the properties and the connections on the right, the views in the
center and the time range at the bottom.

3. Connect and record
---------------------

#. In **Connections** at the bottom right, click **outpost**. Its dot turns
   green, and the menu bar shows how many messages arrive per second.
#. The **Traces & actions** tree on the left fills with what the simulator
   sends, e.g. ``imu``, ``motor``, ``robot/pose`` and ``sysinfo/cpu``.
#. Click **Record** in the bottom panel. blip keeps the values and log records
   only while recording; before, it only learns which traces and actions
   there are.

The log view starts to fill with log records of the components ``sensor``,
``net``, ``storage`` and ``power``.

4. Plot something
-----------------

#. Click the plot (or **Plot 1** in the **Layout** panel) to select it. The
   tree on the left now chooses what this plot shows.
#. Check ``sysinfo`` › ``cpu``: one line per CPU core. Checking a branch also
   adds what arrives below it later.
#. Expand ``imu`` and check ``.acc``: the three elements of the acceleration
   array, ``imu.acc[0]`` to ``imu.acc[2]``.
#. Hover the plot to read the values, scroll the mouse wheel over it to change
   the window length, drag it sideways to look back. Dragging pauses; click
   **Live** in the bottom panel to follow the newest data again.

The **Properties** panel on the right shows the settings of the selected plot,
e.g. fixed y limits, step lines or where the legend goes.

5. Add more views
-----------------

Click **+** in the **Layout** panel and pick a view type. The new view is
added next to the selected one; select it and check its traces:

* **States**: ``motor`` › ``.state`` and ``.enabled`` show as coloured bands,
  one per run of equal values.
* **XY**: ``robot/pose`` › ``.x`` and ``.y`` draw the figure eight the robot
  drives. Choose which trace is on the x axis in the properties.
* **Heatmap**: ``thermal/frame`` is an 8×8 array, shown as a grid at the time
  cursor; ``audio/spectrum`` is a 64-bin array, shown over time.
* **Table**: the values of the checked traces at the time cursor.

Drag a view by its title onto the side of another view to arrange them, see
:doc:`blip/layout`.

.. figure:: /screenshots/window-sensors.png
   :target: ../_images/window-sensors.png
   :class: screenshot
   :alt: The Sensors tab: the robot's figure eight in an XY view, the thermal camera as a heatmap grid and the audio spectrum as a heatmap over time.

   XY and heatmap views of the simulator.

6. Carry out actions
--------------------

#. Add an **Actions** view and select it. The tree now offers the actions the
   simulator announced, marked with a lightning.
#. Check ``motor`` › ``enable`` and ``speed``. The view shows a checkbox and a
   slider, and the current value next to each (the readback).
#. Drag the speed slider. The simulator rejects the setpoint while the motor
   is off: the row shows a warning, hover it for the reason. Check **enable**
   and drag again; plot ``motor`` › ``.speed_rpm`` to watch the motor follow.
#. Check ``imu`` › ``calibrate``: it asks for confirmation before it is sent.

Every invocation also appears in the log view, with the component
``actions``. See :doc:`blip/actions`.

7. Inspect and keep what you saw
--------------------------------

* Click a plot to put the **time cursor** there: tables, heatmaps and the XY
  marker show the values at its time. **Live** returns it to the end.
* The **timeline** at the bottom shows when data arrived and the log records by
  level; drag its thumb to move through the recording. See :doc:`blip/time`.
* **File › Save recording** (:kbd:`Ctrl+S`) saves everything recorded to a
  ``.blip`` file; **File › Open recording** (:kbd:`Ctrl+O`) shows it again
  later, or drop the file onto the window.
* **File › Save layout** saves the views and their settings to a JSON file.
  blip also keeps the layout, the window length and the connections across
  restarts on its own.

Next steps
----------

* :doc:`concepts` -- the ideas behind traces, records, the time range and
  actions.
* :doc:`outpost` -- connect your own target: write a protocol class in Python,
  or pass through a target that speaks the blip protocol.
* :doc:`sonde` -- trace from firmware or unit tests in C++.
