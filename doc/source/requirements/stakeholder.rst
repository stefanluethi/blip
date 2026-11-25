Stakeholder Requirements
========================

.. stk:: Observe live target behaviour
   :id: STK_LIVE_OBSERVE
   :status: approved
   :priority: high

   As a firmware developer I want to observe measured values and log output
   of an embedded target in real time, so that I can debug its behaviour
   without a dedicated debugger.

.. stk:: Zero-install access
   :id: STK_ZERO_INSTALL
   :status: approved
   :priority: medium

   As a developer I want to use the tool from a web browser without
   installing software on my machine.

.. stk:: Connect to common device interfaces
   :id: STK_DEVICE_IF
   :status: approved
   :priority: high

   As a developer I want to connect to targets over serial ports and network
   sockets, and to work without hardware using a simulator.

.. stk:: Control the target
   :id: STK_CONTROL
   :status: draft
   :priority: medium

   As a developer I want to send commands to the target from the tool.

.. stk:: Review observations later
   :id: STK_REVIEW
   :status: approved
   :priority: medium

   As a developer I want to keep what I observed, look at it again later and
   share it with others, together with the arrangement of views I used.

.. stk:: Learn and look up
   :id: STK_DOCS
   :status: draft
   :priority: high

   As a developer I want documentation that gets me going without hardware,
   shows me how to do my task, lets me look up every control and option, and
   helps me when something does not work, for the version I use.

.. stk:: Values with their meaning
   :id: STK_MEANING
   :status: implemented
   :priority: medium

   As a firmware developer I want to see values with the meaning the target
   gives them, such as the name of a state, its unit, what it measures and the
   range it is expected in, so that I need not look them up elsewhere.

.. stk:: Build my own tracing UI
   :id: STK_OWN_UI
   :status: approved
   :priority: medium

   As a Rust developer I want to build my own tracing UI from blip's parts,
   instead of only configuring the shipped blip: an egui application with the
   views, the timeline and the time controls I choose, in code and without
   the traces panel or the properties panel, with widgets of my own, and with
   actions mixed into the views, such as a button in a table column. The
   shipped blip is the reference of an application built that way.

.. stk:: Use my own data
   :id: STK_OWN_DATA
   :status: approved
   :priority: medium

   As a Rust developer I want to feed the views from my own data source in
   my own process, without a separate outpost, and to keep the samples in
   another time series store than the one in memory, such as InfluxDB.
