Logs
====

.. figure:: /screenshots/view-logs.png
   :target: ../../_images/view-logs.png
   :class: screenshot
   :alt: A log view with times, coloured levels, components and messages.

A table of the received log records with time, level, component and message,
of the paths checked in the tree (see :doc:`/blip/traces`); a new log view shows
all. While the shown records are of several paths, a **Path** column tells them
apart. Records outside the shown time range are greyed out. The filters are in the
column headers and are kept with the view:

* **Level** and **Component** -- checkboxes for what is shown, with **All**
  and **None**. New components are shown. Only the components of the shown
  paths are listed.
* **Message** -- a search, as a regular expression; **Aa** matches case. Text
  that is no valid regular expression is searched as it is.

Below the table, the arrow button follows the newest records, the count says
how many of the records of the shown paths are shown, and **Clear filters**
shows all again.

Properties: **Columns** -- **Date** shows the date with the time (of times in
Unix time, not of times since boot), and **Component** shows the component
column. While it is hidden, so is its filter, which then hides nothing.
