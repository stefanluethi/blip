Connecting
==========

The **connections** list the outposts and targets by their name and WebSocket
address. Click the connection chip at the right of the menu bar to open them;
they stay open while you connect, add or edit, and close on a click elsewhere
in the window or on :kbd:`Escape`. It starts with
``ws://127.0.0.1:3000/ws/outpost``, named ``outpost``, where an outpost started
with its defaults listens. Several connections can be open at once.

.. admonition:: Before you begin

   An outpost running, e.g. the simulator of the :doc:`/start/quickstart`, or one of
   your own, see :doc:`/outpost/index`.

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
  also the trace metadata and actions that **Clear data** keeps, and the traces panel
  starts anew. Save a recording you want to keep first: connecting does not
  ask.
* The received values and log records are kept only while recording, started
  with **Record** in the bottom panel once connected, see :doc:`/blip/time`. Before,
  blip only collects the trace metadata and actions the targets announce.
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
   * - amber
     - connected, but the target speaks a newer protocol than blip, see below
   * - red
     - connecting failed or the connection broke; hover it for the reason

A target that sends a ``Hello`` is shown in its row, see below. The reason is also shown as a notice in the bottom right of the views, as is a
connection closed by its outpost or target. The chip in the menu bar shows the
open connection (or their number) and the messages received per second, or
*Not connected*.

What the target says of itself
------------------------------

.. figure:: /screenshots/popup-connections.png
   :target: ../_images/popup-connections.png
   :class: screenshot
   :width: 330px
   :alt: The connections: bench with the target name rig-7 and the line sonde 1.2, time since boot; lab in amber with protocol 2 is newer than blip; and spare, not connected.

   Two connected targets, one on a newer protocol, and one not connected.

A target or outpost sends a ``Hello`` when it connects. The row of its
connection shows what it says, in faint text:

* the **name of the target** at the right of the first line, unless it is the
  name of the connection;
* on the last line the **sender**, the software and version that sends, e.g.
  ``sonde 1.2``, and its **clock**, *time of day* or *time since boot*.

The lines stay, fainter, after the connection closes, until the target sends
another ``Hello`` or you remove the connection. Targets that send no ``Hello``
show no lines.

If the ``Hello`` is of a newer protocol than blip reads (*protocol 2 is newer
than blip*), blip warns once with a notice for each time you connect, colours the
dot in the row and in the chip amber, and says so on the last line and on
hover. It keeps reading and shows what it knows; update blip to see all of it.

Several outposts
----------------

The paths of the traces, the paths of the logs and of the
actions are shown as the outposts send them, e.g. ``cpu/load``, without the
name of the connection: organizing them is up to you, e.g. by the paths each
outpost sends. Traces of the same path from several connections are one
trace, its samples in the order of their timestamps, as are the log records.

Several outposts may announce an action at the same path; an actions view
sends it to the outpost chosen in its properties, see :doc:`/blip/actions`. A saved
recording keeps the name of the connection of each message.

Each target sends its own timestamps. Targets sending the time since boot
rather than the time of day are shown next to each other only as far as they
were started at the same time. If one connection says it sends the time of day
and another the time since boot, blip warns, shows the time of day and removes
the data of the time since boot, see :doc:`/blip/time`.

.. tip::

   In the web app served over ``https``, browsers only allow connections to
   ``wss://`` addresses and to the same machine. See
   :doc:`/start/installation`.

Next steps
----------

* :doc:`/blip/time` -- start recording to keep what arrives.
* :doc:`/blip/traces` -- choose what a view shows.
* :doc:`/help/troubleshooting` -- when blip does not connect.
