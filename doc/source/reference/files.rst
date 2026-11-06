File formats
============

The files you keep, share and maybe generate with other tools.

Recording files (``.blip``)
---------------------------

The messages as received, a CBOR sequence compressed with gzip. blip saves
them, the outpost and sonde write them, blip opens them and the outpost
replays them. The format, with its versions, is described in
:ref:`recording-files`.

Presets files (``.json``)
-------------------------

One or more presets, each a layout of the views with all their settings and
the connections, exported with **Presets › Export presets** and imported with
**Presets › Import presets** or by dropping the file onto blip (see
:doc:`/blip/presets`). It is JSON, so it can be kept in a repository next to
the firmware, compared, edited and generated. blip keeps its own presets in
the same format.

.. code-block:: json

   {
     "version": 1,
     "presets": [
       {
         "name": "Bench",
         "connections": [
           {"name": "outpost", "address": "ws://127.0.0.1:3000/ws/outpost"}
         ],
         "layout": {
           "name": "",
           "kind": "Tabs",
           "active": 0,
           "children": [
             {"Group": {
               "name": "Overview",
               "kind": "Vertical",
               "shares": [2.0, 1.0],
               "children": [
                 {"View": {"name": "Motor speed",
                           "kind": {"Plot": {"traces": ["motor.speed_rpm"], "interpolation": "Step"}}}},
                 {"View": {"name": "Log", "kind": {"Logs": {}}, "wildcards": [""]}}
               ]
             }}
           ]
         }
       }
     ]
   }

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Key
     - Meaning
   * - ``version``
     - The version of the format, see below.
   * - ``presets``
     - The presets, at least one.
   * - Preset ``name``
     - The name of the preset, not blank; the names in a file differ.
   * - Preset ``connections``
     - The connections, each with its ``name`` and ``address``; left out,
       none. The name is the source of the messages of the connection and
       may be empty, see :doc:`/blip/connecting`.
   * - Preset ``layout``
     - The group of the tabs at the top level; left out, an empty layout.
   * - Group ``name``
     - The name shown on its tab; empty for the top level.
   * - Group ``kind``
     - ``Tabs``, ``Vertical`` (stacked) or ``Horizontal`` (side by side).
   * - Group ``children``
     - Its groups and views, each ``{"Group": {...}}`` or ``{"View": {...}}``.
   * - Group ``shares``
     - The relative sizes of the children of a vertical or horizontal group;
       left out, they share equally.
   * - Group ``active``
     - The index of the child shown in a tab group; left out, the first.
   * - View ``name``
     - The title of the view.
   * - View ``kind``
     - The type of view, one of ``Plot``, ``States``, ``Table``, ``Xy``,
       ``Heatmap``, ``Logs`` and ``Actions``, with its settings, e.g.
       ``{"Plot": {"traces": ["motor.speed_rpm"]}}``.
   * - View ``formats``
     - The display format of traces by path: ``Hex``, ``Binary`` or
       ``Scientific``, e.g. ``{"status.flags": "Hex"}``.
   * - View ``wildcards``
     - The branches of the traces tree whose traces are all shown, also those
       that arrive later, by path; ``""`` is the root.

The settings of each type of view are those of its properties (see
:doc:`/blip/views`), e.g. ``traces`` (the paths shown), ``interpolation``
of a plot, ``xs``, ``against`` and ``equal_axes`` of an XY view, ``mode`` of
a heatmap, ``layout`` of a table, ``shown`` of an actions view. A setting
left out takes its default, so a generated preset only needs what differs.
To find the key of a setting, change it in blip and export the preset.

Versions
~~~~~~~~

blip opens presets files of its own and of older versions and migrates them;
it refuses a file of a newer version (*… holds presets of a newer version of
blip*). It always exports the newest version.

.. list-table::
   :header-rows: 1
   :widths: 15 85

   * - Version
     - Change
   * - 1
     - The first.

Layout files of the development versions before presets are no longer
opened.
