Time
====

The time cursor, recording and playback.

.. req:: Time cursor
   :id: REQ_VIS_CURSOR
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   blip shall show one time cursor in all views over time, which the user can
   set and drag, and which stays at the end of the shown time range until set.
   The user shall be able to release it again, with Escape or with Live, and
   it shall then follow the end of the range as before.

.. req:: Record, pause and play
   :id: REQ_VIS_PLAYBACK
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE, STK_REVIEW

   blip shall keep the received values and log records only while recording,
   which the user switches on and off without closing the connections. A
   recording shall start only while a connection is open, and starting it
   shall clear the data received before, after asking while that is an unsaved
   recording (:need:`REQ_FILE_UNSAVED`). blip shall let the user pause the
   shown time range and play it from there at the pace of the timestamps
   while recording goes on.

.. req:: Go live
   :id: REQ_VIS_LIVE
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE
   :rationale: An opened recording has no newest data to follow, but the user
      still wants to get back to its end and to let go of the cursor.

   The button Live shall follow the newest data while recording, and in an
   opened recording, or while not recording, show the end of the data
   ("Follow the end of the recording"). It shall be available whenever it
   changes something, also while the time cursor is set, and shall release the
   cursor. While recording and not live the time panel shall say how far the
   shown range is behind the newest data, such as "Paused, 12 s behind".

.. req:: Play and pause
   :id: REQ_VIS_PLAY
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   The button of the time panel shall show Pause while the shown range moves on,
   following the newest data while recording or playing from where it was paused,
   and Play otherwise. It shall be disabled without data, and in a recording at
   its end, where its hover shall say to move the range back to play from
   there.

.. req:: Length of the shown time range
   :id: REQ_VIS_WINDOW
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   The length of the shown time range shall be one field, with a menu of round
   lengths from 10 ms to 1 h at its caret. The field shall read numbers with
   µs or us, ms, s or sec, m or min, h or hr and d, in any case, and apply
   the length when the user leaves the field. Text it cannot read shall stay
   in the field, outlined in the error colour, with the accepted forms on
   hover and the length unchanged. A length beyond the shortest or the longest
   shall be limited and marked in the same way, with the limits on hover.
   Leaving the field unchanged shall not undo a length set meanwhile.

.. req:: Reading the timeline
   :id: REQ_VIS_TIMELINE
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   Pressing on the ruler of the timeline inside the shown range shall set the
   time cursor and keep the range. A gap without data shall say "No data for"
   and its length on hover. The pill of the hover and the first label of a
   ruler that starts a new day shall have the date. The timeline without data
   shall say what to do: "Connect to an outpost or open a recording" while not
   connected, "Press Record to collect data" while connected, "Waiting for
   data" while recording.

.. req:: Record asks for attention
   :id: REQ_VIS_RECORD_HINT
   :status: implemented
   :component: blip, blip-kit
   :satisfies: STK_LIVE_OBSERVE, STK_CONFIDENCE
   :rationale: Samples arrive but are dropped until Record is pressed, which a
      new user does not know. Amber means a problem on every status dot, and a
      small dot does not draw the eye, so the whole button is emphasised.

   While a connection is open and blip is not recording, with or without data
   shown, the Record button shall be emphasised: filled with the strong text
   colour, with its label and its dot in the background colour and the dot in
   the colour of the label, never amber. Its hover shall say that the incoming
   data is dropped until Record is pressed when no data is shown, and that a
   click clears the data and records when data is shown. While recording, the
   button shall have a red label and dot, a red tint and a red border, and its
   hover shall say that a click stops. While no connection is open and blip is
   not recording, the button shall be plain and disabled.
