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

Floats cannot be shown as states; plot them instead.

Properties: **Bands** -- **Height** of a band as a fraction of its row, and
**Values** to label the bands wide enough with their value.
