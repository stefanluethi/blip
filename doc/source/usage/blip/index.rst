blip
====

blip shows what targets send and carries out their actions. It runs in the
browser and as a native app, with the same features, see
:doc:`/usage/installation`.

The window
----------

.. figure:: /screenshots/window.png
   :target: ../../_images/window.png
   :class: screenshot
   :alt: blip connected to an outpost, recording: plots of the acceleration and the motor speed, the states of the motor, the log, the traces tree and the properties of the selected plot, and the timeline.

   blip with the demo recording and layout.

.. figure:: /diagrams/blip-window.svg
   :target: ../../_images/blip-window.svg
   :class: diagram
   :alt: The blip window: menu on top, traces on the left,
         properties and connections on the right, views in the center, record
         button, time controls and timeline at the bottom.

   The parts of the window.

① Menu bar
   The **File**, **View** and **Help** menus (see :doc:`files`), a chip with
   the open connections and the messages received per second, buttons to hide
   or show the side panels, and the theme button, which switches between the
   light, dark and system theme.

② Traces & actions
   The traces and actions received, as a tree, with a checkbox for each to
   choose what the selected view shows. See :doc:`traces`.

③ Properties
   The settings of the selected view or group. See :doc:`layout` and, for the
   settings of each view type, :doc:`views`.

④ Connections
   The WebSocket addresses of outposts and targets, to connect to them. See
   :doc:`connecting`.

⑤ Views
   The plots, tables, logs and other views, in tabs. Right-click the title of
   a view to add a view next to it or to delete it. See :doc:`layout`.
   Errors and warnings, e.g. when connecting fails, show as notices in the
   bottom right corner of this area.

⑥ Time range
   **Record**, play and pause, **Live**, the length of the window and the
   timeline of all collected data. See :doc:`time`.

Drag the border between the properties and the connections to resize them. Dragging the inner edge of a side panel
past its smallest width hides it, as does its button in the menu bar.

The usual workflow
------------------

#. Connect to one or more outposts (:doc:`connecting`).
#. Start **Record** to keep what arrives (:doc:`time`).
#. Select a view and check its traces in the tree (:doc:`traces`); add and
   arrange views as needed (:doc:`layout`, :doc:`views`).
#. Pause, scroll back, set the time cursor to inspect (:doc:`time`).
#. Invoke actions on the target from an actions view (:doc:`actions`).
#. Save the recording and the layout (:doc:`files`).

.. toctree::
   :maxdepth: 2

   connecting
   traces
   views
   actions
   layout
   time
   files
   reference
