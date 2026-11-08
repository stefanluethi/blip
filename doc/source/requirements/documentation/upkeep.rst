Keeping it right
================

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
