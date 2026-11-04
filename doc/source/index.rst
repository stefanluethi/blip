blip documentation
==================

**blip** is a lightweight tracing tool for embedded systems. It shows what a
target sends -- values, records, arrays and log records -- live as plots,
states, tables, heatmaps and logs, and lets you carry out the actions the target
offers, e.g. set a motor setpoint. It runs in the browser or as a native app.

.. figure:: /screenshots/window.png
   :target: _images/window.png
   :class: screenshot
   :alt: blip connected to an outpost, recording: plots of the acceleration and the motor speed, the states of the motor, the log, the traces tree, the layout and the properties of the selected plot, and the timeline.

   blip showing the outpost's simulator.

blip is one of three parts. The **outpost** runs next to the target and passes
what it sends on to blip; **sonde** traces from firmware and unit tests. How
they fit together is in :doc:`overview/index`.

Where to start
--------------

.. list-table::
   :header-rows: 1
   :widths: 40 60

   * - You want to …
     - Read
   * - see blip working, without hardware
     - :doc:`start/quickstart`
   * - know what the parts are and which you need
     - :doc:`overview/index`, then :doc:`overview/concepts`
   * - look at and record the data of a target
     - :doc:`blip/index`
   * - connect your own target from Python
     - :doc:`outpost/index`
   * - trace from firmware or unit tests in C++
     - :doc:`sonde/index`
   * - fix something that does not work
     - :doc:`help/troubleshooting`
   * - look up an option, a function or a file format
     - :doc:`reference/index`
   * - change blip, the outpost or sonde
     - the *internals* page of each part, e.g. :doc:`blip/internals`, and
       :doc:`requirements/index`

.. toctree::
   :caption: Getting started
   :maxdepth: 1

   start/installation
   start/quickstart

.. toctree::
   :caption: Overview
   :maxdepth: 1

   overview/index
   overview/concepts

.. toctree::
   :caption: The parts
   :maxdepth: 2

   blip/index
   outpost/index
   sonde/index

.. toctree::
   :caption: Help
   :maxdepth: 1

   help/troubleshooting
   help/limits

.. toctree::
   :caption: Reference
   :maxdepth: 2

   reference/index

.. toctree::
   :caption: Project
   :maxdepth: 1

   requirements/index
   changes
