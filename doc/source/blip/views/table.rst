Table
=====

.. figure:: /screenshots/view-table.png
   :target: ../../_images/view-table.png
   :class: screenshot
   :alt: A table of values at the time cursor, with units and arrays.

The values of the checked traces at the time cursor, i.e. the latest sample
at or before it. Arrays show a few elements per dimension; hover them for
more.

Properties, **Rows**:

* **Layout** -- *Traces*: a row per trace. *Records*: a row per record and a
  column per field, e.g. ``acc.z``; the traces panel then checks whole
  records, and a checked branch adds the records that arrive below it later.
* **Traces** (*Traces* layout) -- list them as a *Tree*, by their full
  *Paths*, or by their *Names* only.
* **Empty** (*Traces* layout) -- also show traces without a value at the
  cursor time.

With the *Records* layout, the **Columns** section shows and hides the fields,
reorders them by dragging, and sets the number format of each. Fields that
arrive later are shown.
