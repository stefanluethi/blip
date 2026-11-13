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
