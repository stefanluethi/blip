Presets
=======

A **preset** is a named set of the layout of the views, with all their
settings, and the connections, with their names and addresses. Keep one per
project, target or test bench and switch between them. One preset is active:
the views and connections you see are its own, and changing them changes it.
blip keeps all presets across restarts and starts with the one that was
active. The first start has one preset, *Default*, with an empty layout and a
connection to an outpost on this machine.

Presets menu
------------

The **Presets** menu lists the presets; the active one is highlighted. Click
another to switch to it.

**New preset**
   Adds a preset named *Preset 1* (or the next free number), with an empty
   layout and the default connection, and switches to it.

**Duplicate preset**
   Adds a copy of the active preset, named after it with *copy*, and switches
   to it, e.g. to try out a layout and keep the original.

**Rename preset…**
   Renames the active preset. The names of the presets differ.

**Delete preset…**
   Deletes the active preset after asking, and switches to the next one. The
   only preset cannot be deleted.

**Import presets…**, **Export presets…**
   Import presets from a file and export them to one, see below.

Switching presets disconnects all connections; connect again from the
connection chip (see :doc:`/blip/connecting`). The data received stays. While
recording, the active preset stays: switching, adding, duplicating and
deleting are disabled. Stop recording first.

Exporting and importing
-----------------------

**Presets › Export presets…** asks which presets to export, all are checked
at first, and saves them to one JSON file, ``blip-presets.json`` by default.
In the browser, the file is downloaded. Keep it in a repository next to the
firmware, share it with the team or generate it, see :doc:`/reference/files`.

**Presets › Import presets…** opens a presets file, as does dropping one onto
the window or picking one with **File › Open recording**. blip lists the
presets in the file, all checked: uncheck those to leave out. A preset with
the name of one you have is marked *replaces the existing one*; importing it
replaces that preset, with its layout and connections. Replacing the active
preset shows the imported one at once, so it is disabled while recording.

blip opens presets files of its own and of older versions. A file of a newer
version of blip is refused, see :doc:`/help/troubleshooting`.

Next steps
----------

* :doc:`/blip/layout` -- arrange the views of a preset.
* :doc:`/blip/connecting` -- the connections of a preset.
* :doc:`/reference/files` -- the presets file, to keep, compare or generate.
