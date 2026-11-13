States
======

.. figure:: /screenshots/view-states.png
   :target: ../../_images/view-states.png
   :class: screenshot
   :alt: States of the motor and the log level as coloured bands.

The values of strings, bools and integers over time, as a row of coloured
bands per trace, one band for each run of equal values, e.g. the state of a
state machine. Runs too short to tell apart are drawn as one band showing
that the value changes too often. Hover for a tooltip with the time and the state of each
trace, a faint *—* for a trace without a sample; click to put the time cursor there.

Each value keeps its colour: a light band under an edge in the full colour.
A ``false`` bool leaves its band empty, so ``true`` reads as on, and log
levels such as ``DEBUG`` or ``ERROR`` take the colours of the log view.

.. figure:: /screenshots/view-states-labels.png
   :target: ../../_images/view-states-labels.png
   :class: screenshot
   :alt: States of a motor state and of a mode trace whose bands read Idle, Ramp, Run and Fault.

A trace with labels from the target, e.g. the mode of a controller sent as
``0``, ``1``, ``2``, shows each band with its label (*Idle*, *Ramp*, *Run*)
in place of the number. The colour of a band still follows the number, so each
number keeps its colour, unless the label is the name of a log level, which
takes the level's colour. Choose another number format in **Traces** to see
the numbers.

.. figure:: /screenshots/view-states-labels-tooltip.png
   :target: ../../_images/view-states-labels-tooltip.png
   :class: screenshot
   :alt: A states tooltip with the label Run and the number 2 faint beside it, and below it the description of the trace.

The tooltip shows the label, the number faint beside it, and below a rule the
name and description of the trace under the pointer, if the target sent one.

Floats cannot be shown as states; plot them instead.

Properties: **Bands** -- **Height** of a band as a fraction of its row, and
**Values** to label the bands wide enough with their value.
