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

.. test:: Tooltips of the plot-like views
   :id: TEST_BLIP_TOOLTIP
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_TOOLTIP, REQ_VIS_DIST_BOX_HOVER, REQ_VIS_HIST_HOVER

   ``blip/src/app/view/tooltip`` unit tests (``egui_kittest``) -- the shared
   tooltip helper shows a header (a text in weak colour, or a trace dot and
   name), a row per entry with the dot and name or a weak label on the left, and
   the values right aligned in columns as wide as the widest value, with the
   unit, a missing or null value in the faint colour, and a rule between two
   groups of rows.
   ``blip/src/app/view/plot`` unit tests (``egui_kittest``) -- hovering Plot
   shows the time as header and a row per trace with its value, through the
   helper.
   ``blip/src/app/view/states`` unit tests (``egui_kittest``) -- hovering States
   shows the time as header and a row per trace with its state, through the
   helper.
   ``blip/src/app/view/heatmap`` unit tests (``egui_kittest``) -- hovering the
   bands of a Heatmap shows the time as header and the element index with its
   value, and hovering the grid the row and column as header and the value,
   through the helper.
   ``blip/src/app/view/heatmap`` unit tests (``egui_kittest``) -- hovering the
   array line of a Heatmap shows the element index and its value in a framed
   tooltip, and no label of egui_plot.
   ``blip/src/app/view/xy`` unit tests (``egui_kittest``) -- hovering XY shows
   the name of the trace with its x and y values in a framed tooltip, and no
   label of egui_plot.
   ``blip/src/app/view/distribution`` unit tests (``egui_kittest``) -- the
   tooltips of a bin and of a box of Distribution are built through the helper,
   with the bin range or the dot and name of the trace as header.

.. test:: Tooltip screenshots
   :id: TEST_BLIP_TOOLTIP_SHOTS
   :status: implemented
   :component: blip
   :verifies: REQ_VIS_TOOLTIP

   ``blip/src/app/screenshots.rs::screenshots_views`` renders
   ``view-plot-tooltip`` and ``view-heatmap-grid-tooltip``, a tooltip of Plot and
   one of a Heatmap, in the light and the dark theme, for the documentation and
   to check by eye that they look like the tooltip of a box of Distribution
   (``view-distribution-box-tooltip``).
