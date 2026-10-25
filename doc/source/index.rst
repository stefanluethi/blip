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

Three parts work together:

* **blip**, the app you look at (Rust, egui; web and native).
* **outpost**, a small Python service next to the target. It talks to the target
  in whatever protocol the target speaks and serves blip over a WebSocket. It
  also simulates a target, records and replays.
* **sonde**, a C++17 library for firmware and unit tests that encodes traces as
  blip messages, to send them to an outpost or write them to a recording file.

.. figure:: /diagrams/system-context.svg
   :target: _images/system-context.svg
   :class: diagram
   :alt: The target, optionally with sonde, talks to the outpost; the outpost
         serves blip over a WebSocket and can record to a file; unit tests with
         sonde write recording files that blip opens.

   How the parts fit together.

Where to start
--------------

* New to blip? Follow the :doc:`usage/quickstart`: a simulated target on the
  screen in a few minutes.
* :doc:`usage/concepts` explains the words used everywhere: traces, records,
  views, the time range, recordings and actions.
* The guides for each part: :doc:`usage/blip/index`, :doc:`usage/outpost` and
  :doc:`usage/sonde`.
* :doc:`architecture/index` describes how the software is built, for those
  who change it or connect their own targets.
* :doc:`requirements/index` lists what the software must do, with traceability
  managed by `sphinx-needs <https://sphinx-needs.readthedocs.io>`_.

.. toctree::
   :maxdepth: 2
   :caption: Contents

   usage/index
   architecture/index
   requirements/index
