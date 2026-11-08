Structure
=========

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

.. req:: Overview of the parts
   :id: REQ_DOC_OVERVIEW
   :status: implemented
   :component: doc
   :priority: high
   :satisfies: STK_DOCS
   :rationale: The Kubernetes *Components* page and Zephyr's introduction:
               readers must know what each part is for before they can pick
               the guide to read.

   Before the guide of any part, the documentation shall show the parts --
   blip, the outpost and sonde -- what each is for, how they connect, and
   which setup fits which situation.

.. req:: Organised by part
   :id: REQ_DOC_PARTS
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: Zephyr keeps the overview, usage and API of each subsystem
               together: readers work with one part at a time, users and
               contributors alike.

   The pages of each part shall be together: its guides, then how it is built
   inside, with its reference linked from them. Concepts shared by all parts
   shall be apart from the guide of any one part.

.. req:: Each topic in one place
   :id: REQ_DOC_ONE_PLACE
   :status: implemented
   :component: doc
   :priority: medium
   :satisfies: STK_DOCS
   :rationale: GitLab's single source of truth for its docs: a topic told
               twice drifts apart, and readers cannot tell which is right.

   Each topic shall be described on one page; other pages link to it rather
   than repeat it, and a page for users and a page for contributors on the
   same topic shall each say only what their reader needs.
