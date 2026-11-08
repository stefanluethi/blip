Versions and changes
====================

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
