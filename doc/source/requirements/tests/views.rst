Views
=====

Tests of the requirements in :doc:`/requirements/system/views`.

.. test:: Views compute what they show
   :id: TEST_BLIP_VIEWS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_STATES, REQ_VIS_TABLE, REQ_VIS_XY, REQ_VIS_HEATMAP

   ``blip/src/app/view`` unit tests -- merge equal values into state runs
   with gaps at nulls, hide collapsed rows of the table, pair samples with the
   current sample of their x trace, pair sibling x and y traces, sample arrays
   over time for heatmaps, and zoom plots with the mouse wheel without panning
   them, unless disabled.

.. test:: Log records are kept by path and shown in a table
   :id: TEST_BLIP_LOGS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_LOG

   ``blip/src/app/model/logs.rs`` and ``blip/src/app/view/log_table.rs`` unit
   tests -- keep records by path in the order of their times, changing the
   order of a path only when a record arrives before others of it; show the
   records of the picked paths merged by time, filter only new records unless
   the filter or the paths change, filter by level, component and message, show
   the path while several are shown, and show or hide the date and the
   component as the properties say.

.. test:: Integers are kept exactly
   :id: TEST_BLIP_NUMBERS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_INT

   ``blip/src/app/model/numbers.rs`` and ``blip/src/app/model/traces.rs``
   tests -- keep integers beyond 2^53 exact, switch to unsigned for integers
   above ``i64::MAX``, reject integers that do not fit with the others, and
   write them in every number format.
