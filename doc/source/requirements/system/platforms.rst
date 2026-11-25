Platforms
=========

Where blip runs and what it keeps across restarts.

.. req:: Web application
   :id: REQ_PLAT_WEB
   :status: implemented
   :component: blip
   :satisfies: STK_ZERO_INSTALL

   blip shall run in current desktop web browsers as a WebAssembly
   application. In a browser without WebGL it shall say "blip needs WebGL,
   which this browser does not offer."

.. req:: Native application
   :id: REQ_PLAT_NATIVE
   :status: implemented
   :component: blip

   blip shall be buildable as a native desktop application on Linux, whose
   window opens 1280 × 800 points large and cannot be made smaller than
   800 × 500.

.. req:: Persist UI state
   :id: REQ_PERSIST_STATE
   :status: implemented
   :component: blip

   blip shall persist the presets, each with the layout of the views and the
   connections, and which preset is active across restarts.

.. req:: Presets
   :id: REQ_PRESETS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall keep named presets, each with the layout of the views and the
   connections, show those of the active preset, and switch between them
   while not recording. Changes of the layout and the connections shall apply
   to the active preset. A new or duplicated preset, and switching to another,
   shall keep the open connections that the new preset also has, with the same
   name and address, and disconnect the others.

.. req:: Presets menu and dialogs
   :id: REQ_PRESETS_DIALOGS
   :status: implemented
   :component: blip, blip-kit
   :satisfies: STK_LIVE_OBSERVE, STK_CONFIDENCE

   The dialog to rename a preset shall open with its name selected. The dialog
   to delete one shall be confirmed with Enter, once it has been shown, and
   cancelled with Escape. While the kept presets are of a newer version of blip
   and changes are not kept, the menu shall start with "Changes to presets are
   not kept".
