Using the UI
============

Connecting
----------

#. Enter the WebSocket address of the bridge or target in the settings panel,
   e.g. ``ws://127.0.0.1:3000/ws/bridge``.
#. Press **Connect**. The button changes to **Disconnect** while connected.

Views
-----

The window is split into tiled panes that can be rearranged and grouped into
tabs:

Plot
   Time-series plot of all received ``Value`` streams, keyed by ``path``. The
   streams are selected per plot from a tree that branches at each ``/`` of
   the path.

Log table
   Table of received ``Log`` records with timestamp, level, component and
   message.

The layout and settings (including the target address) are persisted and
restored on the next start.

.. todo::

   Add screenshots of the plot and log views.
