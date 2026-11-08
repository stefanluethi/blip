Presentation
============

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
