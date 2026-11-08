Content
=======

What each kind of page shall hold.

Getting started
---------------

.. req:: Installation with exact prerequisites
   :id: REQ_DOC_INSTALL
   :status: implemented
   :component: doc
   :priority: high
   :satisfies: STK_DOCS
   :rationale: Zephyr's *Getting Started Guide* and the Rust install page:
               naming the versions and tools up front saves a failed build
               half way through.

   The installation guide shall list what each part needs, with the minimum
   versions, and the commands to build or install each part.

.. req:: Tutorial without hardware
   :id: REQ_DOC_TUTORIAL
   :status: implemented
   :component: doc
   :priority: high
   :satisfies: STK_DOCS
   :rationale: The Django polls tutorial and the guessing game of the Rust
               Book: one complete, working result early builds confidence.
               Most readers do not have the target at hand.

   A tutorial shall take a new user from nothing to a simulated target shown
   in blip, with each step saying what to expect, in a few minutes and
   without hardware.

.. req:: Before you begin and next steps
   :id: REQ_DOC_NEXT
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: Kubernetes task pages: readers land on a page from a search,
               not from the page before.

   Task pages shall state what they assume (installed parts, a running
   outpost, …) at the top and link to what to read next at the end.

How-to guides
-------------

.. req:: Guides for common tasks
   :id: REQ_DOC_TASKS
   :status: implemented
   :component: doc
   :priority: high
   :satisfies: STK_DOCS
   :rationale: GitLab task topics and GNOME help: readers come with a goal,
               such as connecting their own target, not with a feature name.

   The documentation shall have step by step guides for the common tasks:
   connecting an own target to an outpost, tracing from firmware and from
   unit tests with sonde, recording, replaying and sharing a recording with
   its preset, and invoking actions.

.. req:: Troubleshooting
   :id: REQ_DOC_TROUBLESHOOT
   :status: implemented
   :component: doc
   :priority: high
   :satisfies: STK_DOCS
   :rationale: GitLab *Troubleshooting* sections, the Arch Wiki and Zephyr:
               people search for the message they see, so the page must
               contain it.

   The documentation shall describe the common problems by their symptom,
   quoting the error and warning messages word for word, with their cause and
   fix: blip does not connect, no data arrives, timestamps or rates look
   wrong, a recording or presets file does not open.

Reference
---------

.. req:: Reference of the user interface
   :id: REQ_DOC_UI_REF
   :status: implemented
   :component: doc
   :priority: high
   :satisfies: STK_DOCS
   :rationale: The Blender manual and the Wireshark User's Guide describe
               every menu, panel and tool, so nothing in the window is left
               to guessing.

   The reference shall describe every menu, panel, view type and setting of
   blip, and list all its mouse and keyboard controls.

.. req:: Reference of the command line
   :id: REQ_DOC_CLI_REF
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: curl documents every option and fails a test when one is
               missing from the docs; git's manual pages match ``--help``.

   Every command and option of ``blip-outpost`` shall be documented with its
   default, and a test shall fail when an option of ``--help`` is missing
   from the documentation.

.. req:: Reference of the libraries
   :id: REQ_DOC_API_REF
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: The Python library reference, docs.rs and the NumPy docstring
               standard: generated from the code, the reference cannot drift
               from it.

   The public API of the outpost (Python) and of sonde (C++) shall be
   documented with each class and function, its parameters with their units,
   what it returns and the errors it raises, taken from the docstrings and
   comments in the code.

.. req:: File formats for users
   :id: REQ_DOC_FORMATS
   :status: implemented
   :component: doc
   :priority: low
   :satisfies: STK_DOCS
   :rationale: Grafana's dashboard JSON model: users keep dashboards in their
               repositories, review and edit them, and generate them.

   The documentation shall describe the formats users keep and exchange: the
   recording file, the presets file with its versions, and the wire protocol,
   in enough detail to write or read them with other tools.

.. req:: Terms defined once
   :id: REQ_DOC_TERMS
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: The Kubernetes glossary: one meaning per word, so a trace is
               not also a stream or a signal on another page.

   The documentation shall define its terms (target, outpost, trace, record,
   view, recording, action, …) in one place and use them the same way on
   every page.

Explanation
-----------

.. req:: Concepts explained
   :id: REQ_DOC_CONCEPTS
   :status: implemented
   :component: doc
   :priority: high
   :satisfies: STK_DOCS
   :rationale: Diátaxis explanation and Django's *topics*: knowing how time,
               the window and recordings work explains what the controls do.

   The documentation shall explain the ideas behind blip: why the outpost
   sits next to the target, how traces and records are formed from paths,
   and how timestamps, the time range, the cursor and recording work.

Examples
--------

.. req:: Tested examples
   :id: REQ_DOC_EXAMPLES
   :status: draft
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: rustdoc compiles and runs the examples in the docs and Python
               has ``doctest``: an example that does not work costs the reader
               more than no example.

   Code examples in the documentation shall be complete enough to run, and
   the examples of the outpost and sonde shall be run or compiled by the
   tests, ideally included into the pages from the files that are tested.

.. req:: Expected results shown
   :id: REQ_DOC_RESULTS
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: The Rust Book shows the output after each command, so readers
               can tell that a step worked.

   After a command or a step, the documentation shall show or describe what
   the reader sees: the output, the log line, or a screenshot of blip.
