Documentation Requirements
==========================

What the user documentation of blip, the outpost and sonde must offer. The
expectations are taken from open source projects whose documentation is known
to work well for their users; each requirement names where it comes from in
its rationale.

Sources
-------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Project
     - What blip takes from it
   * - `Diátaxis <https://diataxis.fr>`_, used by Django, NumPy and the
       Canonical docs
     - Four kinds of documentation, each with its own job: tutorials, how-to
       guides, reference and explanation.
   * - `Django <https://docs.djangoproject.com>`_
     - A tutorial that builds one real thing end to end; release notes for
       every release, with what to change when upgrading.
   * - `Python <https://docs.python.org>`_
     - Tutorial and reference kept apart; *What's New* per release and
       ``versionadded`` / ``versionchanged`` notes; *Security considerations*
       sections; downloadable docs; *Report a bug* on every page.
   * - `The Rust Book <https://doc.rust-lang.org/book/>`_ and rustdoc
     - The output shown after each command; examples in the docs compiled and
       run as tests.
   * - `Kubernetes <https://kubernetes.io/docs/home/>`_
     - *Before you begin* and *What's next* on task pages; a glossary;
       documentation as part of finishing a feature.
   * - `GitLab <https://docs.gitlab.com>`_
     - Topic types (concept, task, reference, troubleshooting);
       *Troubleshooting* sections that start from the symptom; a style guide
       checked by a linter (Vale); docs in the definition of done.
   * - `Blender manual <https://docs.blender.org/manual/en/latest/>`_ and the
       `Wireshark User's Guide <https://www.wireshark.org/docs/wsug_html/>`_
     - A reference for every menu, panel and tool of a GUI, with its keyboard
       and mouse controls.
   * - `curl <https://curl.se/docs/>`_
     - Every command line option documented, and a test that fails when an
       option is missing from the docs.
   * - `Zephyr <https://docs.zephyrproject.org/latest/>`_
     - Getting started per host OS with the exact versions needed; lists of
       what is supported; troubleshooting for the common setup problems.
   * - `Grafana <https://grafana.com/docs/>`_
     - Documentation for the version in use; the file formats users edit
       (the dashboard JSON model) described for users.
   * - `Google developer documentation style guide
       <https://developers.google.com/style>`_
     - Second person, present tense, active voice, sentence case headings,
       alt text for every image.

Structure
---------

.. req:: Four kinds of documentation
   :id: REQ_DOC_KINDS
   :status: implemented
   :component: doc
   :priority: high
   :satisfies: STK_DOCS
   :rationale: Diátaxis, as used by Django and NumPy: a page that mixes
               learning, doing, looking up and understanding serves none of
               them well.

   The documentation shall offer a tutorial, how-to guides, reference and
   explanation, and each page shall serve mainly one of them.

.. req:: Where to start
   :id: REQ_DOC_ENTRY
   :status: implemented
   :component: doc
   :priority: high
   :satisfies: STK_DOCS
   :rationale: Django's *First steps* and the Kubernetes docs home: readers
               arrive with different goals and need a way in for theirs.

   The first page shall say what blip is, show it, and point each kind of
   reader (first time, connecting a target, tracing firmware, changing blip)
   to where they start.

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
   its layout, and invoking actions.

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
   wrong, a recording or layout does not open.

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
   :rationale: Grafana's dashboard JSON model: users keep layouts in their
               repositories, review and edit them, and generate them.

   The documentation shall describe the formats users keep and exchange: the
   recording file, the layout file with its versions, and the wire protocol,
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

Versions and changes
--------------------

.. req:: Documentation of the version in use
   :id: REQ_DOC_VERSION
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: The Python version switcher and Grafana's versioned docs: a
               page about a newer release misleads users of an older one.

   The documentation shall show the version it describes, and blip's Help
   menu shall show blip's version and link to the documentation. Once
   releases are kept apart, the documentation of each release shall stay
   available.

.. req:: Release notes
   :id: REQ_DOC_CHANGES
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: Django release notes, Python's *What's New* and Keep a
               Changelog: users need to know what changed and what to do
               about it before they upgrade.

   Each release shall have release notes listing what is new, changed and
   removed, and what users must change, e.g. in layout or recording files.
   From 1.0 on, pages shall mark when a feature was added or changed.

Finding things
--------------

.. req:: Search and stable links
   :id: REQ_DOC_FIND
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: Read the Docs and Sphinx projects in general: readers search,
               and links in issues, chats and code comments must keep
               working.

   The documentation shall be searchable, every page shall be reachable from
   the navigation, and a page that moves shall leave a redirect.

.. req:: Offline documentation
   :id: REQ_DOC_OFFLINE
   :status: implemented
   :component: doc
   :priority: low
   :satisfies: STK_DOCS
   :rationale: Python's downloadable docs and Godot's offline docs; labs with
               embedded targets are often cut off from the internet.

   The documentation shall be readable without internet access, built from
   the repository with the steps it describes, without fonts or scripts
   loaded from other sites.

Presentation
------------

.. req:: Current, accessible images
   :id: REQ_DOC_IMAGES
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: Google's style guide asks for alt text on every image; GUI
               docs with stale screenshots, a common complaint, lose trust.

   Screenshots shall be rendered from blip by a build step, so they can be
   renewed with each change of the window, in the light and the dark theme.
   Every image shall have alt text that says what it shows, and no
   information shall be carried by colour alone.

.. req:: Plain style
   :id: REQ_DOC_STYLE
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: The Google developer documentation and GitLab style guides:
               short, direct sentences are faster to read and to translate
               with a tool.

   The documentation shall address the reader as *you*, in the present tense
   and the active voice, with plain words, sentence case headings, units on
   every number and UI labels written as they appear in the window.

.. req:: Limits and safety
   :id: REQ_DOC_LIMITS
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: Python's *Security considerations* sections and Zephyr's lists
               of supported boards: users should learn the limits from the
               docs, not from the target.

   The documentation shall state the known limits (supported platforms and
   browsers, message rates, queues and the size of recordings) and what to
   take care of: the outpost has no authentication, binding it to ``0.0.0.0`` serves
   anyone on the network, and actions act on real hardware.

Keeping it right
----------------

.. req:: Documentation with the code
   :id: REQ_DOC_WITH_CODE
   :status: implemented
   :component: doc
   :priority: high
   :satisfies: STK_DOCS
   :rationale: GitLab's definition of done and the Kubernetes feature
               process: a change that users see is not finished until it is
               documented.

   Changes that users see shall update the documentation in the same
   commit, kept in the repository next to the code.

.. req:: Clean build
   :id: REQ_DOC_BUILD
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: Read the Docs' ``fail_on_warning`` and Sphinx's ``-W``: broken
               references are found when they are made, not by readers.

   The documentation shall build without warnings, and the build shall run
   for every change published.

.. req:: Working links
   :id: REQ_DOC_LINKS
   :status: implemented
   :component: doc
   :priority: low
   :satisfies: STK_DOCS
   :rationale: Sphinx's ``linkcheck`` builder, run by many Sphinx projects in
               CI: external links rot silently.

   The links to other sites shall be checked automatically, at least before
   each release.

.. req:: Feedback from each page
   :id: REQ_DOC_FEEDBACK
   :status: implemented
   :component: doc
   :priority: low
   :satisfies: STK_DOCS
   :rationale: Python's *Report a bug* and the *Edit this page* links of
               Kubernetes: the reader who finds a mistake is the one to fix
               it, if it takes one click.

   Each page shall link to its source in the repository and to where
   problems are reported.

Not taken over
--------------

Some practices of the projects above do not pay off for blip:

Translations (Blender, KiCad)
   Embedded developers read English documentation; translations that lag
   behind the English pages mislead more than they help.
Video tutorials
   They go stale with every change of the window and cannot be searched;
   the generated screenshots stay current.
A wiki (Arch Linux, sigrok)
   Pages edited outside the repository drift from the version of the code;
   the documentation lives with the code instead (:need:`REQ_DOC_WITH_CODE`).
Documentation for each release before 1.0
   Until there are releases that users stay on, one version is enough
   (:need:`REQ_DOC_VERSION`).

Where it stands
---------------

.. needtable::
   :types: req
   :filter: component == "doc"
   :columns: id, title, status, priority
   :style: table
   :sort: id

Where each is met:

* :need:`REQ_DOC_KINDS` -- the tutorial is the :doc:`/usage/quickstart`, the
  how-to guides are the pages of blip, the outpost and sonde, the reference is
  :doc:`/usage/reference/index`, the explanation is :doc:`/usage/concepts` and
  :doc:`/architecture/index`.
* :need:`REQ_DOC_TROUBLESHOOT` -- :doc:`/usage/troubleshooting`.
* :need:`REQ_DOC_NEXT` -- *Before you begin* and *Next steps* on the guides.
* :need:`REQ_DOC_CLI_REF` -- :doc:`/usage/reference/outpost-cli`, checked by
  :need:`TEST_DOC_CLI`.
* :need:`REQ_DOC_API_REF` -- :doc:`/usage/reference/outpost-api` from the
  docstrings, :doc:`/usage/reference/sonde-api` from the headers.
* :need:`REQ_DOC_FORMATS` -- :doc:`/usage/reference/files` and
  :doc:`/architecture/protocol`.
* :need:`REQ_DOC_CHANGES` -- :doc:`/changes`.
* :need:`REQ_DOC_LIMITS` -- :doc:`/usage/limits`.
* :need:`REQ_DOC_FIND` -- ``redirects`` in ``conf.py``, for pages that move.
* :need:`REQ_DOC_LINKS` -- ``make linkcheck``.
* :need:`REQ_DOC_FEEDBACK` -- *Edit this page* and *Report a problem* beside
  each page.

Still open:

* :need:`REQ_DOC_EXAMPLES` -- the Python examples of the outpost guide are
  compiled and the ``minimal`` example it includes is run by
  :need:`TEST_DOC_EXAMPLES`, but the C++ snippets of the sonde guide are not
  compiled.

:doc:`/usage/limits` gives no measured message rate of blip itself, as it depends
on the machine and the views; the ``fast`` example measures it.
