Actions
=======

Targets announce the actions they can carry out, e.g. set a motor setpoint,
change a log level or start a calibration. An **actions view** is a form to
carry them out. blip only sends an action when you do: never on its own, at
startup or when it connects again.

.. figure:: /diagrams/action-sequence.svg
   :target: ../../_images/action-sequence.svg
   :class: diagram
   :alt: Sequence: the outpost announces the actions; the user changes an
         input; blip sends Invoke; the outpost checks the arguments and calls
         the action, which talks to the target; the outpost answers with an
         ActionResult; the readback trace shows the new state.

   An invocation from start to end.

Choosing the actions
--------------------

Add an actions view (see :doc:`layout`) and select it. The traces panel then
offers the announced actions, marked with a lightning; check those the view
shows. A new actions view shows none.

Each action is a row: its name (hover it for the description, the path and
the outpost it is sent to), its input, the current value of its readback
trace if it has one, and what became of its latest invocation.

Inputs
------

.. figure:: /screenshots/view-actions.png
   :target: ../../_images/view-actions.png
   :class: screenshot
   :alt: An actions view: a checkbox, two sliders with their readbacks, a choice, an action with two parameters and a Send button, and a Run button.

* An action without parameters is a **Run** button.
* An action with one parameter is sent as soon as its input changes:

  * a checkbox for a bool,
  * buttons or a dropdown for a choice,
  * a slider for a number with a minimum and a maximum, sent while it is
    dragged (at most ten times a second, and the final value on release),
  * a number field otherwise, sent when dragged like a slider, or once a
    typed number is entered with :kbd:`Enter` or by leaving the field,
  * a text field for a string, sent with :kbd:`Enter`.

  Clicking a field to type in it sends nothing. Integers are typed in a text
  field and kept exact, also beyond 2\ :sup:`53`.
* An action with several parameters lists them below its row and is sent with
  its **Send** button, or, if chosen under **Sending** in the properties, each
  time one of its inputs changes (at most ten times a second, the latest
  change sent once the time is up).
* An action that asks for confirmation is sent after confirming it in a
  dialog.

The inputs start from the readback, the default of the parameter or its lowest
value. Checkboxes and choices with a readback follow it, so they show the
state of the target. After a change they keep the new value until a readback
sample arrives that is newer than the target's answer.

Outcome
-------

The last column shows what became of the latest invocation:

.. list-table::
   :widths: 25 75

   * - spinner
     - the target has not answered yet
   * - check mark
     - carried out (shown for a moment)
   * - warning
     - rejected; hover it for the reason
   * - clock
     - no answer within five seconds

Whether the action had the desired effect shows in the traces, e.g. the
readback.

When actions are disabled
-------------------------

An action is disabled while the connection of its outpost is not connected,
e.g. while a recording is open, and while its target cannot carry it out, e.g.
during a calibration. Checked actions that no outpost announces, or that the
chosen outpost does not announce, are listed greyed out; the traces panel lists
them too, to uncheck them.

Properties
----------

* **Outpost** -- the outpost all actions of the view are sent to, among those
  that announced actions. **First announcing**, the default, sends each action
  to the first outpost announcing it whose connection is open, else to the
  first announcing it, so the actions keep working when connecting to an
  outpost of another name. A chosen outpost that announces no actions any more
  is listed greyed out. The answers of an action, and whether it is
  connected, are those of the outpost it is sent to.
* **Readbacks** -- for each action whether it shows the readback its outpost
  announces, which the readback column shows and checkboxes and choices
  follow. The checkbox is disabled if the outpost announces none.
* **Sending** -- for each action with several parameters whether it is sent
  with its *Send button* or *On change* of any of its inputs. Actions that ask
  for confirmation always keep their Send button.
* **Number inputs** -- switch numbers with a minimum and a maximum between a
  *Slider* and a *Field*.

To offer actions from your own target, see :ref:`outpost-actions`.
