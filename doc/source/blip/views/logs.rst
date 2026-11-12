Logs
====

.. figure:: /screenshots/view-logs.png
   :target: ../../_images/view-logs.png
   :class: screenshot
   :alt: A log view with times, coloured levels, paths and messages.

A table of the received log records with time, level, path and message,
of the paths checked in the tree (see :doc:`/blip/traces`); a new log view shows
all. The **Path** column tells the records apart; its cells are relative to the
folder the shown paths share, e.g. ``imu`` and ``motor/left`` for ``log/imu``
and ``log/motor/left`` (hover a cell for the full path). Records outside the shown time range are greyed out. The filters are in the
column headers and are kept with the view:

* **Level** and **Path** -- checkboxes for what is shown, with **All** and
  **None**. The paths are listed flat. Unchecked paths are kept with the
  layout, so new paths are shown.
* **Message** -- a search, as a regular expression; **Aa** matches case. Text
  that is no valid regular expression is searched as it is.

.. figure:: /screenshots/view-logs-path-filter.png
   :target: ../../_images/view-logs-path-filter.png
   :class: screenshot
   :alt: The Path filter of a log view, with a checkbox per log path.

Below the table, the arrow button follows the newest records, the count says
how many of the records of the shown paths are shown, and **Clear filters**
shows all again.

Properties: **Columns** -- **Date** shows the date with the time (of times in
Unix time, not of times since boot).
