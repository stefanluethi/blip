Documentation
=============

Tests of the requirements in :doc:`/requirements/documentation/index`.

.. test:: Documentation builds without warnings
   :id: TEST_DOC_BUILD
   :status: implemented
   :component: doc
   :verifies: REQ_DOC_BUILD

   ``cd doc && make html`` -- builds with ``-W --keep-going``, so a broken
   reference or a malformed directive fails the build; the Pages workflow
   (``.github/workflows/pages.yml``) runs it for each published change.

.. test:: Every option of the outpost command line is documented
   :id: TEST_DOC_CLI
   :status: implemented
   :component: doc
   :verifies: REQ_DOC_CLI_REF

   ``outpost/tests/test_docs.py::test_every_option_is_documented`` and
   ``::test_every_command_is_documented`` -- go through the commands and options
   of ``blip-outpost`` and check that each is on the page of the command line.

.. test:: The examples of the outpost guide work
   :id: TEST_DOC_EXAMPLES
   :status: implemented
   :component: doc
   :verifies: REQ_DOC_EXAMPLES

   ``outpost/tests/test_docs.py::test_the_python_examples_of_the_guide_compile``
   and ``::test_the_minimal_example_sends_values`` -- compile each Python code
   block of the outpost guide, and run ``examples/minimal.py``, which the guide
   includes, until it sends a value to a connected client.

.. test:: Links to other sites work
   :id: TEST_DOC_LINKS
   :status: implemented
   :component: doc
   :verifies: REQ_DOC_LINKS

   ``cd doc && make linkcheck`` -- requests each link to another site and fails
   on broken ones; run before each release.

.. test:: The protocol reference covers every message
   :id: TEST_DOC_PROTOCOL
   :status: implemented
   :component: doc
   :verifies: REQ_DOC_FORMATS

   ``outpost/tests/test_docs.py::test_the_protocol_page_covers_every_message_and_field``
   -- goes through the variants of ``Message`` and the fields of ``Hello``,
   ``Sample``, ``Samples``, ``TraceMetadata`` and ``Log`` in ``proto/src`` and
   checks that each is on the page of the protocol reference;
   ``::test_the_protocol_page_names_the_recording_versions`` checks that its
   section on recording files names versions 1 to 3.
