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

.. req:: Found by search engines
   :id: REQ_DOC_SEARCH_ENGINES
   :status: implemented
   :component: doc, blip
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: Readers look for a tool or an answer in a search engine before
               they know the address of the documentation; a sitemap, one
               canonical address and a description per page are what the
               engines ask for.

   The published site shall have a sitemap listing the web app and every page
   of the documentation, announced in its ``robots.txt``. Each of these pages
   shall state its language, its canonical address and a description, and the
   tags for the preview of a link to it. Redirects and generated pages
   (search, index) shall be left out of the sitemap.

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
