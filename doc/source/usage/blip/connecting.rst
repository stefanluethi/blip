Connecting
==========

The **connections** list the outposts and targets by their name and WebSocket
address. Click the connection chip at the right of the menu bar to open them;
they stay open while you connect, add or edit, and close on a click elsewhere
in the window or on :kbd:`Escape`. It starts with
``ws://127.0.0.1:3000/ws/outpost``, named ``outpost``, where an outpost started
with its defaults listens. Several connections can be open at once.

Adding and editing connections
------------------------------

* Add an address with the **+** button; :kbd:`Enter` keeps it, :kbd:`Escape`
  drops it. The new connection is named after the last part of the path of the
  address, e.g. ``robby`` for ``ws://192.168.1.10/ws/robby``, numbered if
  another connection has that name.
* Right-click a connection to connect or disconnect it, to rename it, to edit
  its address or to delete it. Rename it while it is disconnected. The names
  of two connections differ, unless they are empty.

Connecting and disconnecting
----------------------------

* Click a connection to connect it, click it again to disconnect it.
* The connections are independent: connecting another one keeps the data
  received so far; clear it with **File › Clear data**.
* A connection that opens while no other is open **starts afresh**: the data,
  traces and actions received before, or of an opened recording, are dropped,
  also the units and actions that **Clear data** keeps, and the traces panel
  starts anew. Save a recording you want to keep first: connecting does not
  ask.
* The received values and log records are kept only while recording, started
  with **Record** in the bottom panel once connected, see :doc:`time`. Before,
  blip only collects the units and actions the targets announce.
* While recording, the connections cannot be changed: they cannot be
  connected, disconnected, added, edited or deleted. Stop recording to change
  them. Only a connection that was connected when the recording started and
  broke can be connected again, and the recording goes on.

Connection state
----------------

The dot before each connection shows its state:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Dot
     - State
   * - hollow
     - disconnected
   * - spinner
     - connecting
   * - green
     - connected
   * - red
     - connecting failed or the connection broke; hover it for the reason

The reason is also shown as a notice in the bottom right of the views, as is a
connection closed by its outpost or target. The chip in the menu bar shows the
open connection (or their number) and the messages received per second, or
*Not connected*.

Several outposts
----------------

The paths of the traces, the components of the logs and the paths of the
actions are shown as the outposts send them, e.g. ``cpu/load``, without the
name of the connection: organizing them is up to you, e.g. by the paths each
outpost sends. Traces of the same path from several connections are one
trace, its samples in the order of their timestamps, as are the log records.

Several outposts may announce an action at the same path; an actions view
sends it to the outpost chosen in its properties, see :doc:`actions`. A saved
recording keeps the name of the connection of each message.

Each target sends its own timestamps. Targets sending the time since boot
rather than the time of day are shown next to each other only as far as they
were started at the same time.

.. tip::

   In the web app served over ``https``, browsers only allow connections to
   ``wss://`` addresses and to the same machine. See
   :doc:`/usage/installation`.
