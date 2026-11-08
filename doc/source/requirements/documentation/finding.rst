Finding things
==============

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
