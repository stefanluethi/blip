Menu, files and settings
========================

File menu
---------

**Open recording** (:kbd:`Ctrl+O`)
   Disconnects all connections, stops recording and shows the data of a
   recording file as if it had been received, in place of all data, traces and
   actions before, with the time range fitted to all of it and the traces
   panel started anew. A file that was cut off, e.g. because the outpost was
   killed, opens up to its last whole message, with a warning.

**Save recording** (:kbd:`Ctrl+S`)
   Saves all messages received since the data was last cleared to a ``.blip``
   file named after the current time, with the names of their connections,
   also those of an opened recording. In the browser, the file is downloaded.

**Clear data**
   Removes all received data but the latest units and actions of the targets,
   which they may send only when connecting or when they change. The recording
   starts again with them, so a saved recording has them too.

**Open layout**, **Save layout**
   Open and save the layout of the views, with all their settings, as a JSON
   file, see :doc:`layout`.

**Reset layout**
   Returns to a tab with a plot and a log view.

Recording and layout files can also be **dropped onto the window**; blip tells
them apart by their contents.

Recording files are also written by the outpost (``record``, see
:doc:`/usage/outpost`) and by sonde's ``FileSink`` (see :doc:`/usage/sonde`),
and the outpost replays them. Their format is described in
:doc:`/architecture/protocol`.

View menu
---------

Zooms the whole UI in and out, also with :kbd:`Ctrl` and :kbd:`+`, :kbd:`-`
and :kbd:`0` in the native app. In the browser, these keys zoom the page. The
zoom is kept across restarts.

Help menu
---------

The version of blip and a link to this documentation.

What blip remembers
-------------------

blip keeps, across restarts:

* the layout, with all settings of the views,
* the length of the window,
* the connections, their names and addresses,
* the zoom, the theme and whether the side panels are hidden.

It does not keep the received data: save a recording to keep it. The native
app stores its settings in the configuration directory of the user, the web
app in the local storage of the browser for the site it is served from.
