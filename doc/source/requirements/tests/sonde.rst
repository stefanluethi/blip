Sonde
=====

Tests of the requirements in :doc:`/requirements/system/sonde`.

.. test:: sonde sends a Hello
   :id: TEST_SONDE_HELLO
   :status: implemented
   :component: sonde
   :verifies: REQ_SONDE_HELLO

   ``sonde/tests/test_hello.cpp`` (all cases) and ``sonde/tests/test_file_sink.cpp``
   ("a file sink writes a recording") -- checks that a queue given a ``Hello``
   sends it once, before its first message of any kind, with or without a sink,
   with the given name and clock (left out when empty or unknown) and the sender
   ``sonde 0.2.0``, that a queue without one sends the default ``Hello`` (version
   and sender only), that a ``Hello`` too large for a batch is dropped once, and
   that a recording begins with it. ``proto/tests/sonde.rs`` decodes it from the
   fixture.

.. test:: sonde sends metadata
   :id: TEST_SONDE_METADATA
   :status: implemented
   :component: sonde
   :verifies: REQ_SONDE_METADATA

   ``sonde/tests/test_encoding.cpp`` ("trace metadata leaves out what is not declared",
   "a unit is trace metadata"), ``sonde/tests/test_tracer.cpp`` ("a tracer sends
   metadata below its name") -- checks the encoded ``TraceMetadata`` with the
   path and each of unit, description, labels (negative and wide values, in
   order) and range (as floats), that undeclared ones are left out and that a
   tracer joins the path. ``proto/tests/sonde.rs`` decodes the fixture's
   metadata.

.. test:: sonde sends blocks of samples
   :id: TEST_SONDE_SAMPLES
   :status: implemented
   :component: sonde
   :verifies: REQ_SONDE_SAMPLES

   ``sonde/tests/test_encoding.cpp`` ("samples at a period", "samples at
   timestamps", "samples need as many timestamps as values"),
   ``sonde/tests/test_tracer.cpp`` ("a tracer sends samples below its name") and
   ``sonde/tests/test_batches.cpp`` ("a block larger than a batch is dropped")
   -- checks that a block with a period and one with timestamps encode as one
   ``Samples`` with exactly one of them, from C arrays, ``std::array``, ``Span``
   and ``array()``, also empty and with null values, that unequal lengths are
   ``Invalid`` and nothing is sent, that a tracer joins the path and that a block
   larger than a batch is dropped as ``TooLarge``; proto decodes the fixture's
   blocks.
