Control
=======

Actions that targets announce and blip invokes.

.. req:: Announce actions
   :id: REQ_CTRL_ANNOUNCE
   :status: implemented
   :component: proto, outpost
   :satisfies: STK_CONTROL

   The target shall announce the actions it can carry out with their
   parameters (bool, integer, float, choice or string, with bounds, unit and
   default), an optional readback trace, whether they need confirmation and
   whether they are enabled. Each announcement shall replace the previous one,
   and blip clients connecting later shall get the latest.

.. req:: Invoke actions
   :id: REQ_CTRL_ACTION
   :status: implemented
   :component: blip, outpost
   :satisfies: STK_CONTROL

   blip shall send invocations of announced actions with their arguments to
   the connected target. The outpost shall check the arguments against the
   parameters and carry out the invocations of an action one after another in
   the order they arrived.

.. req:: Action feedback
   :id: REQ_CTRL_FEEDBACK
   :status: implemented
   :component: blip, outpost
   :satisfies: STK_CONTROL

   The target shall answer each invocation to the client that sent it with
   whether it carried it out or the reason it rejected it. blip shall show
   whether the latest invocation of an action is pending, accepted, rejected
   with the reason, or unanswered after a timeout. The outpost shall not
   send log records about invocations.

.. req:: Actions view
   :id: REQ_CTRL_VIEW
   :status: implemented
   :component: blip
   :satisfies: STK_CONTROL

   blip shall offer a view of actions placed with the layout, showing buttons,
   checkboxes, choices, sliders, number and text fields by the kinds of their
   parameters, and the value of their readback traces. Sliders shall send
   while dragged at a limited rate, actions with several parameters shall be
   sent with a separate Send button. The user shall choose the actions a view
   shows in the tree of the traces, where actions are marked as such, per
   view the connection the actions are sent to, and per action whether it has
   the readback trace announced.

.. req:: Actions only on request
   :id: REQ_CTRL_SAFE
   :status: implemented
   :component: blip
   :satisfies: STK_CONTROL

   blip shall send an action only when the user operates its input, never on
   its own at startup or on reconnecting, ask before sending actions that
   need confirmation, and disable actions while not connected or while the
   target cannot carry them out.
