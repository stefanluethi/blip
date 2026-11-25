Menu, files and settings
========================

File menu
---------

**Open recording** (:kbd:`Ctrl+O`)
   Reads a recording file. Until it is read, the connections and a running
   recording stay as they are, and cancelling the dialog leaves everything as
   it was. Pressing **Record** while the file is read cancels the opening:
   *Opening <file> was cancelled by recording*. Once read, blip disconnects all
   connections, stops recording and shows the data of the file as if it had
   been received, in place of all data, traces and actions before, with the
   time range fitted to all of it and the traces panel started anew. The
   name of the file shows in the connections chip and, natively, in the title
   of the window. A file that was cut off, e.g. because the outpost was
   killed, opens up to its last whole message, with a warning. Opening a
   recording also clears the failures of the connections that the chip shows.

**Save recording** (:kbd:`Ctrl+S`)
   Saves all messages received since the data was last cleared to a ``.blip``
   file named after the current time, with the names of their connections,
   also those of an opened recording. In the browser, the file is downloaded,
   and the menu item has no ellipsis, as it asks for nothing. blip tells *Saved
   <name>* in a notice; with nothing recorded, :kbd:`Ctrl+S` says *Nothing
   recorded to save*.

**Clear recording**
   Removes all received data but the latest trace metadata and actions of the targets,
   which they may send only when connecting or when they change. The recording
   starts again with them, so a saved recording has them too. It is disabled
   without data.

Discarding an unsaved recording
-------------------------------

A recording is **unsaved** from its first recorded message until you save it,
clear it or replace it. blip asks before it drops an unsaved recording: *Discard
the unsaved recording?* with **Discard** and **Cancel**, and :kbd:`Escape`
cancels. It asks before **Record**, **File › Clear recording**, opening a
recording and dropping one onto the window, and when you quit: *Quit and
discard the unsaved recording?*. The browser warns when you reload or leave the page.

Connecting does not wipe an unsaved recording: it stays, and the logs of the
new connection are not added to it. A recording you opened is not unsaved until
you record anew.

Recording and presets files can also be **dropped onto the window**; blip
tells them apart by their contents. A dropped presets file is imported, see
:doc:`/blip/presets`.

Recording files are also written by the outpost (``record``, see
:doc:`/outpost/index`) and by sonde's ``FileSink`` (see :doc:`/sonde/index`),
and the outpost replays them. Their format is described in
:doc:`/reference/protocol`.

View menu
---------

**Add view** adds a view of the chosen type after the selected view, and
**Clear layout** removes all views and tabs of the active preset, keeping its
connections, see :doc:`/blip/layout`. **Undo delete** brings back the view, tab
or group, or the layout, that was deleted last.

**Zoom in**, **Zoom out** and **Reset zoom** (with the zoom in percent) zoom the
whole UI, also with :kbd:`Ctrl` and :kbd:`+`, :kbd:`-` and :kbd:`0` in the
native app. In the browser, these keys zoom the page. The zoom is kept across
restarts.

Presets menu
------------

The button reads *Presets: <name>* with the active preset. The menu has the
presets to switch between, and **New**, **Duplicate**, **Rename** and
**Delete preset**, **Import presets** and **Export presets**, see
:doc:`/blip/presets`.

Help menu
---------

The version of blip, **Keyboard shortcuts**, see :doc:`/blip/controls`, and a
link to this documentation, which opens in a new tab.

What blip remembers
-------------------

blip keeps, across restarts:

* the presets, each with its layout, with all settings of the views, and its
  connections, their names and addresses,
* which preset is active,
* the length of the window,
* the zoom, the theme and whether the side panels are hidden.

The native window starts at 1280 × 800 points and cannot be made smaller than
800 × 500. The panel buttons in the menu bar say what a click does, e.g. *Hide
the traces* and *Show the properties*, and so does the theme button. A browser
without WebGL shows *blip needs WebGL, which this browser does not offer.*

It does not keep the received data: save a recording to keep it. The native
app stores its settings in the configuration directory of the user, the web
app in the local storage of the browser for the site it is served from.

Next steps
----------

* :doc:`/reference/files` -- the formats of recordings and presets.
* :doc:`/blip/controls` -- all mouse and keyboard controls.
