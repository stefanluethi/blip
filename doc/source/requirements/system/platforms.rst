Platforms
=========

Where blip runs and what it keeps across restarts.

.. req:: Web application
   :id: REQ_PLAT_WEB
   :status: implemented
   :component: blip
   :satisfies: STK_ZERO_INSTALL

   blip shall run in current desktop web browsers as a WebAssembly
   application.

.. req:: Native application
   :id: REQ_PLAT_NATIVE
   :status: implemented
   :component: blip

   blip shall be buildable as a native desktop application on Linux.

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
   to the active preset.
