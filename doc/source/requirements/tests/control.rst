Control
=======

Tests of the requirements in :doc:`/requirements/system/control`.

.. test:: Action messages encode as plain CBOR
   :id: TEST_PROTO_ACTIONS
   :status: implemented
   :component: proto
   :verifies: REQ_CTRL_ANNOUNCE, REQ_CTRL_FEEDBACK

   ``proto/src/action.rs`` unit tests -- encode invocations and results as
   the documented CBOR maps, parameter kinds as strings or maps, and decode
   announcements whose optional fields are left out.

.. test:: Outpost announces, checks and carries out actions
   :id: TEST_OUTPOST_ACTIONS
   :status: implemented
   :component: outpost
   :verifies: REQ_CTRL_ANNOUNCE, REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK, REQ_OUTPOST_PROTOCOL

   ``outpost/tests/test_actions.py`` -- takes the parameters of actions from
   their type hints and rejects unsupported ones, checks and converts
   arguments, rejects disabled actions and those that raise, sends the
   actions to connecting clients and changed ones to all, answers only the
   invoking client, logs to all, and runs a slow action without holding up
   others.

.. test:: blip invokes actions and shows their outcome
   :id: TEST_BLIP_ACTIONS
   :status: implemented
   :component: blip
   :verifies: REQ_CTRL_ACTION, REQ_CTRL_FEEDBACK, REQ_CTRL_VIEW, REQ_CTRL_SAFE

   ``kit/src/store/actions.rs``, ``kit/src/widgets/view/actions.rs`` and
   ``kit/src/widgets/view/trace_selection.rs`` tests -- send invocations with new
   ids, show only the outcome of the latest invocation of an action of an
   outpost, time out unanswered ones, send at most every interval while
   dragging and the final value on release; send buttons and checkboxes right
   away and actions with several parameters with their Send button, ask before
   confirmed actions, disable actions while disconnected or disabled, and
   start inputs from the readback or the default; send the actions to the
   outpost chosen for the view, by default each to the first announcing it
   with an open connection, show and follow the announced readback unless
   turned off, choose both in the properties, keep the inputs per outpost,
   show no actions in new views until checked, list the actions in the
   tree of the traces, also those shown but not announced, and check them
   there only for actions views.

.. test:: Typed numbers are sent with Enter and a run once
   :id: TEST_BLIP_ACTIONS_SEND
   :status: implemented
   :component: blip
   :verifies: REQ_CTRL_TYPED, REQ_CTRL_PENDING

   ``kit/src/widgets/view/actions/tests.rs`` --
   ``typed_numbers_are_sent_on_enter_only`` and
   ``typed_integers_are_sent_exactly_on_enter`` check that a number typed in a
   field is sent with Enter; ``typed_numbers_left_without_enter_stay_unsent_drafts``
   that integers and floats left by a click elsewhere send nothing, stay marked
   "Changed" with "Enter to send", are sent by Enter in the field, and that an
   Enter without a draft sends nothing; ``typed_integers_are_marked_until_entered``
   the mark; ``a_pending_run_cannot_be_sent_again`` that a double click on Run
   sends once, Run is disabled until the answer, and runs again after it.
