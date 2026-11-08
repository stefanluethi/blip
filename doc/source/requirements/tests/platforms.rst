Platforms
=========

Tests of the requirements in :doc:`/requirements/system/platforms`.

.. test:: Presets are switched and kept
   :id: TEST_BLIP_PRESETS
   :status: implemented
   :component: blip
   :verifies: REQ_PRESETS, REQ_PERSIST_STATE

   ``blip/src/app.rs`` tests -- switching, adding, duplicating and deleting
   presets shows the layout and connections of each, and the presets menu
   does not switch while recording. ``blip/src/app/presets.rs`` tests --
   the presets are kept with the active one, kept presets of a newer version
   are not read.
   ``blip/src/app/connections.rs::connections_of_a_preset_keep_their_addresses_and_names_apart``
   -- the connections of a preset keep their addresses and names apart.
