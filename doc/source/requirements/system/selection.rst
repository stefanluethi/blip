Traces tree
===========

The tree on the left, "Traces, actions and logs", in which the user chooses
what the selected view shows.

.. req:: Reading the traces tree
   :id: REQ_SEL_MARKS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   The tree shall mark a folder that picks the traces below it, also those that
   appear later (a wildcard), with a faint "+" and say on hover whether it is
   picking. A view that shows one trace, such as a Heatmap, shall offer radio
   buttons in place of checkboxes, and in an XY view the hover of a whole array
   shall say that it replaces the other traces. Long names shall end in "…".
   The titles of the panels and of their sections shall be written in sentence
   case, and the left panel shall be called "Traces, actions and logs".

.. req:: Narrowing the traces tree
   :id: REQ_SEL_FILTER
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE

   Besides the search, the tree shall offer a toggle Checked only that shows
   only the checked traces. When the search or a filter hides checked traces it
   shall say how many. A menu ⋯ shall offer Expand all and Collapse all, and an
   Alt+click on the caret of a folder shall open or close everything below it.
   While no view is selected the folders shall still open and close and only
   the checkboxes shall be disabled.

.. req:: Traces tree from the keyboard
   :id: REQ_SEL_KEYS
   :status: implemented
   :component: blip
   :satisfies: STK_LIVE_OBSERVE, STK_CONFIDENCE

   Ctrl+F shall put the focus on the search of the tree, opening the panel if it
   is collapsed. The arrow keys shall move the focus in the tree and open and
   close folders, and Space shall check the row in focus.
