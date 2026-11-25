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
   does not switch while recording. ``kit/src/workbench/presets.rs`` tests --
   the presets are kept with the active one, kept presets of a newer version
   are not read.
   ``kit/src/workbench/connections.rs::connections_of_a_preset_keep_their_addresses_and_names_apart``
   -- the connections of a preset keep their addresses and names apart.
   ``kit/src/workbench/connections/tests.rs::a_preset_keeps_the_open_connections_it_also_has``
   -- a preset keeps the open connections of the same name and address and
   disconnects the others, forgetting the failures of those it drops.
   ``blip/src/app/tests.rs::switching_presets_keeps_the_layout_and_connections_of_each``
   -- switching shows the layout and connections of each preset.

.. test:: The presets menu and its dialogs
   :id: TEST_BLIP_PRESETS_DIALOGS
   :status: implemented
   :component: blip, blip-kit
   :verifies: REQ_PRESETS_DIALOGS

   ``kit/src/workbench/presets/tests.rs`` --
   ``the_rename_field_takes_the_focus_once_with_the_name_selected`` checks that
   typing replaces the selected name and Tab moves on to the buttons;
   ``deleting_a_preset_is_confirmed_with_enter_or_kept_with_escape`` that the
   Enter that opened the dialog does not confirm it, a later Enter does, and
   Escape keeps the preset; ``the_presets_menu_tells_while_changes_are_not_kept``
   that the menu says "Changes to presets are not kept" only while kept presets
   of a newer version are not read.
