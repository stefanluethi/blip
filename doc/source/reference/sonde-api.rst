sonde API
=========

The C++17 library sonde, for firmware and unit tests. How to use it is in
:doc:`/sonde/index`; this page lists all of it. Everything is in the namespace
``sonde``; the declarations below are included from the headers, with their
comments, so they are those of the code.

Summary
-------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Name
     - Purpose
   * - ``Queue``
     - encodes values and log records into batches, each with its full path;
       one per context that traces
   * - ``Tracer``
     - a named part of the code on a queue: paths below its name, log records
       at its name
   * - ``Storage<BatchBytes, Batches = 2>``
     - the batches of one queue, static or on the stack, owned by you
   * - ``Sink``
     - takes the batches of a queue whose producer and consumer are one
       context: implement ``write(data, size)``
   * - ``FileSink``, ``MemorySink``
     - sinks of hosted builds (``sonde/file_sink.hpp``): a recording file, or
       memory for checks in unit tests
   * - ``Batch``
     - a ready batch taken by the consumer: ``data`` and ``size``
   * - ``Status``
     - the result of each call: ``Ok``, ``Full``, ``TooLarge``,
       ``SinkError``, ``Invalid``
   * - ``Level``
     - the levels of log records: ``Trace``, ``Debug``, ``Info``, ``Warn``,
       ``Error``
   * - ``Timestamp``, ``Clock``
     - nanoseconds, and the function that returns the current time
   * - ``system_clock_ns``, ``default_clock``
     - UTC from the system clock (hosted builds), the clock a queue uses by
       default
   * - ``record(n, fill)``, ``array(n, fill)``, ``Span<T>``, ``null``
     - values built by a fill function, a view of elements, a null value;
       the value types are listed in :ref:`sonde-values`
   * - ``Encoder``, ``Record``, ``Array``
     - passed to fill functions and to ``encode`` functions of your own types

The CMake options are listed in :doc:`/sonde/index`.

Queue
-----

From ``sonde/sonde.hpp``:

.. literalinclude:: /../../sonde/include/sonde/sonde.hpp
   :language: cpp
   :start-after: // --- Queue
   :end-before: private:

Tracer
------

From ``sonde/sonde.hpp``:

.. literalinclude:: /../../sonde/include/sonde/sonde.hpp
   :language: cpp
   :start-after: // --- Tracer
   :end-before: private:

Types
-----

From ``sonde/sonde.hpp``:

.. literalinclude:: /../../sonde/include/sonde/sonde.hpp
   :language: cpp
   :start-after: // --- Types
   :end-before: // --- Encoding of values

Sinks
-----

From ``sonde/file_sink.hpp`` (hosted builds only):

.. literalinclude:: /../../sonde/include/sonde/file_sink.hpp
   :language: cpp
   :start-after: namespace sonde {
   :end-before: }  // namespace sonde

The interface of a sink of your own, from ``sonde/sonde.hpp``:

.. literalinclude:: /../../sonde/include/sonde/sonde.hpp
   :language: cpp
   :start-at: /// Takes the batches of a queue whose producer and consumer are one context.
   :end-before: // --- Tracer
