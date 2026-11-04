File formats
============

The files you keep, share and maybe generate with other tools.

Recording files (``.blip``)
---------------------------

The messages as received, a CBOR sequence compressed with gzip. blip saves
them, the outpost and sonde write them, blip opens them and the outpost
replays them. The format, with its versions, is described in
:ref:`recording-files`.

Layout files (``.json``)
------------------------

The views with all their settings, saved with **File › Save layout** and opened
with **File › Open layout** or by dropping the file onto blip (see
:doc:`/blip/files`). It is JSON, so it can be kept in a repository next to
the firmware, compared, edited and generated.

.. code-block:: json

   {
     "version": 5,
     "root": {
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

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Key
     - Meaning
   * - ``version``
     - The version of the format, see below.
   * - ``root``
     - The group of the tabs at the top level.
   * - Group ``name``
     - The name shown on its tab; empty for the root.
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
of a plot, ``x`` and ``equal_axes`` of an XY view, ``mode`` of a heatmap,
``layout`` of a table, ``shown`` of an actions view. A setting left out takes
its default, so a generated layout only needs what differs. To find the key of
a setting, change it in blip and save the layout.

Versions
~~~~~~~~

blip opens layouts of its own and of older versions and migrates them; it
refuses a layout of a newer version (*… is a layout of a newer version of
blip*). It always saves the newest version.

.. list-table::
   :header-rows: 1
   :widths: 15 85

   * - Version
     - Change
   * - 1
     - The first.
   * - 2
     - The x trace of an XY view is one of its traces.
   * - 3
     - The traces of a view are saved as ``traces``; ``streams`` is still
       read.
   * - 4
     - An XY view can have several x traces; each y trace is plotted against
       one of them.
   * - 5
     - Log views show the logs of the paths picked in the tree; the log views
       of older layouts pick all (``"wildcards": [""]``).
