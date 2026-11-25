Application
===========

How the window of blip behaves: keys, notices and labels.

.. req:: Keyboard shortcuts
   :id: REQ_APP_SHORTCUTS
   :status: implemented
   :component: blip
   :satisfies: STK_CONFIDENCE
   :rationale: Recording, playing and going live are done while the hand is on
      the target or the mouse is on a plot.

   blip shall offer these keys, and list them under Help › Keyboard shortcuts: R
   to record or stop, Space to play or pause, L to go live, Escape to release
   the time cursor, Ctrl+F to search the traces, F2 to rename and Delete to
   delete the selected view of the tab shown, Ctrl+O to open a recording and
   Ctrl+S to save it. Escape shall release the time cursor only after it has
   cancelled a field, closed a menu, a dialog or the list of connections. No
   key shall act while a text field has the focus or a dialog is open, and
   Space and Delete shall wait while a menu is open. R shall ask as the Record
   button does (:need:`REQ_FILE_UNSAVED`).

.. req:: Notices
   :id: REQ_APP_NOTICES
   :status: implemented
   :component: blip, blip-kit
   :satisfies: STK_CONFIDENCE, STK_LIVE_OBSERVE

   blip shall tell what happened in notices of three levels: neutral ones that
   confirm, such as "Saved a.blip", warnings and errors. Neutral notices and
   warnings shall disappear on their own, errors shall stay until the user
   closes them, and a notice shall be able to offer a button of its own such as
   Undo. A notice repeated while it is shown shall be counted and shown for
   longer instead of stacked, unless it has a button.

.. req:: Controls say what they do
   :id: REQ_APP_LABELS
   :status: implemented
   :component: blip
   :satisfies: STK_CONFIDENCE

   The button of the Presets menu shall show "Presets: " and the name of the
   active preset. The buttons of the panels shall say "Hide the traces", "Show
   the traces", "Hide the properties" or "Show the properties", and the theme
   button shall name the theme a click switches to. Help › Documentation shall
   open in a new tab.
