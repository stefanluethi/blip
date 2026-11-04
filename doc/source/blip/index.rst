blip
====

blip shows what targets send and carries out their actions. It runs in the
browser and as a native app, with the same features, see
:doc:`/start/installation`.

The window
----------

.. figure:: /screenshots/window.png
   :target: ../_images/window.png
   :class: screenshot
   :alt: blip connected to an outpost, recording: plots of the acceleration and the motor speed, the states of the motor, the log, the traces tree and the properties of the selected plot, and the timeline.

   blip with the demo recording and layout.

.. figure:: /diagrams/blip-window.svg
   :target: ../_images/blip-window.svg
   :class: diagram
   :alt: The blip window: menu with the connection chip on top, traces on the
         left, properties on the right, views in the center, record
         button, time controls and timeline at the bottom.

   The parts of the window.

① Menu bar
   The **File**, **View** and **Help** menus (see :doc:`/blip/files`), a chip with
   the open connections and the messages received per second, which opens the
   connections to the outposts and targets (see :doc:`/blip/connecting`), buttons to hide
   or show the side panels, and the theme button, which switches between the
   light, dark and system theme.

② Traces & actions
   The traces and actions received, as a tree, with a checkbox for each to
   choose what the selected view shows. See :doc:`/blip/traces`.

③ Properties
   The settings of the selected view or group. See :doc:`/blip/layout` and, for the
   settings of each view type, :doc:`/blip/views`.

④ Views
   The plots, tables, logs and other views, in tabs. **⋯** on the title of a
   view adds a view next to it or deletes it. See :doc:`/blip/layout`.
   Errors and warnings, e.g. when connecting fails, show as notices in the
   bottom right corner of this area.

⑤ Time range
   **Record**, play and pause, **Live**, the length of the window and the
   timeline of all collected data. See :doc:`/blip/time`.

Dragging the inner edge of a side panel past its smallest width hides it, as
does its button in the menu bar.

How blip shows data
-------------------

View
   A pane in the center that shows traces, logs or actions: a plot, a states
   view, a table, an XY plot, a heatmap, a log view or an actions view. Each
   view has its own selection and its own settings. See :doc:`views`.

Selection
   What a view shows is chosen in the **Traces & actions** tree while the view
   is selected. Checking a branch also checks what arrives below it later.
   See :doc:`traces`.

Layout
   The views arranged as a tree of groups: the groups at the top level are
   tabs, the groups below show their children as tabs, stacked vertically or
   side by side. The layout is kept across restarts and can be saved to a
   JSON file. See :doc:`layout`.

Recording
   blip is connected all the time but keeps values and log records only while
   **Record** is on; before, it collects only the units and actions the
   targets announce. A recording starts with the data cleared and can be
   saved to a ``.blip`` file and opened again. Actions are carried out only
   while recording, so the recording shows what they did. See :doc:`time`.

Time range
   All views show the same stretch of time: a **window** of a set length, e.g.
   20 s. **Live**, it ends at the newest data and moves on with it.
   **Paused**, it stays where it is; **playing**, it moves on at the pace of
   the timestamps, like playing back a video.

Time cursor
   A point in time shown as a line in all plots over time and on the
   timeline. Tables, heatmaps at the cursor and the XY marker show the values
   at its time. Until you set it, it is at the end of the window.

.. figure:: /diagrams/time-range.svg
   :target: ../_images/time-range.svg
   :class: diagram
   :alt: The shown window over the collected data, with the time cursor, when
         live, paused and playing.

   The time range when live, paused and playing.

The ideas blip shares with the outpost and sonde -- traces, records, log
records, timestamps, actions -- are in :doc:`/overview/concepts`.

The usual workflow
------------------

#. Connect to one or more outposts (:doc:`/blip/connecting`).
#. Start **Record** to keep what arrives (:doc:`/blip/time`).
#. Select a view and check its traces in the tree (:doc:`/blip/traces`); add and
   arrange views as needed (:doc:`/blip/layout`, :doc:`/blip/views`).
#. Pause, scroll back, set the time cursor to inspect (:doc:`/blip/time`).
#. Invoke actions on the target from an actions view (:doc:`/blip/actions`).
#. Save the recording and the layout (:doc:`/blip/files`).

.. toctree::
   :maxdepth: 2

   connecting
   traces
   views
   actions
   layout
   time
   files
   controls
   internals
