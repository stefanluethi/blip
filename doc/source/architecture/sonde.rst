sonde
=====

sonde (``sonde/``) is a C++17 library built with CMake: ``include/sonde`` has
the API (``sonde.hpp``, and ``file_sink.hpp`` for hosted builds), ``src`` the
tracer, the encoder and the file sinks. It encodes with `tinycbor
<https://github.com/intel/tinycbor>`_. How to use it is in :doc:`/usage/sonde`;
``sonde/README.md`` has the full API and the design decisions.

Goals
-----

* Runs on microcontrollers: no heap, no exceptions or RTTI needed, no
  iostreams; the user provides the storage.
* Safe across contexts -- two tasks, an ISR and a task -- without locks, also on
  a Cortex-M0.
* The API follows ``proto``: one call per ``Message`` variant, with the same
  fields in the same order.
* The output is exactly what the outpost and blip read: messages as a CBOR
  sequence, byte for byte the encoding of the ``proto`` crate.

The tracer and its batches
--------------------------

A ``Tracer`` encodes messages into the batches of its ``Storage<BatchBytes,
Batches>``, slots the user owns (static or on the stack). A batch is a CBOR
sequence of whole messages, ready to be one UDP datagram or one write.

.. figure:: /diagrams/sonde-batches.svg
   :target: ../_images/sonde-batches.svg
   :class: diagram
   :alt: The batches cycle from free to open (the producer encodes into it),
         ready (committed), taken (the consumer sends it) and back to free on
         release.

   The batches of a tracer form a single-producer single-consumer queue.

* **Single producer, single consumer.** The producer (the context that traces)
  and the consumer (the context that sends) share two indices,
  ``std::atomic<std::uint32_t>``, each written by one side only, read with
  acquire and written with release. No locks, no compare-and-swap: this works
  from ISRs and on cores without ``LDREX``/``STREX``. sonde ``static_assert``\ s
  that the atomics are lock-free, or, with GCC on 32-bit targets, that their
  loads and stores are.
* **The producer never blocks.** With no free batch, a message is dropped with
  ``Status::Full`` and counted in ``dropped()``, so tracing never changes the
  timing of the traced code because the link is slow.
* **Commits.** A batch becomes ready when the next message does not fit or on
  ``commit()``. The consumer cannot close the open batch, as the producer may be
  encoding into it; ``request_commit()`` increments a counter only the
  consumer writes, and the producer commits when it sees a new count at its
  next message or ``commit()``.
* **With a sink**, producer and consumer are one context: a ready batch goes to
  ``Sink::write`` right away.

Encoding
--------

Each message is encoded by a fresh tinycbor encoder into the free part of the
open batch, so messages are atomic: if one does not fit, the batch keeps its
size, the tracer commits it and encodes the message again into the next free
batch. tinycbor reports how many bytes were missing, so a message larger than a
whole batch is dropped as ``TooLarge`` without committing. The clock is read
only once a message has a batch, so tracing into a full queue costs no clock
read.

Values are encoded through overloads and templates: scalars, strings,
``T[N]``, ``std::array``, ``Span``, ``std::optional``, records and arrays built
by fill functions (template parameters, no ``std::function``), and user types
with an ``encode`` function found by argument-dependent lookup. Floats use the
shortest of 16, 32 and 64 bits that keeps the value; the sign of an integer
decides its CBOR major type -- both as in the ``proto`` crate. Record and array
sizes are given up front and checked by tinycbor when closing; a mismatch ends
the message as ``Invalid`` and leaves the batch unchanged.

The constructors are ``constexpr``, so static tracers are constant-initialized
and ready before any code runs.

File output
-----------

``FileSink`` writes a recording file: the version 1 header and then the
batches, uncompressed; blip and the outpost read uncompressed files too.
``MemorySink`` keeps the bytes for tests.

Tests and CI
------------

The Catch2 tests compare the encoding of each message and kind of value byte
for byte with the ``proto`` crate, check the batch rules (whole messages,
``Full``, ``TooLarge``, commits, drops), run a producer and a consumer on two
threads (also under ThreadSanitizer), and check that ``FileSink`` writes the
fixture ``tests/data/recording.blip``, which a Rust test in ``proto`` decodes
and the outpost reads. CI also builds the library with ``-fno-exceptions
-fno-rtti`` and for Cortex-M0 and M4 with ``arm-none-eabi-gcc``.

Not yet
-------

* **Actions**: announcing actions, decoding ``Invoke`` requests and answering
  them are planned as a second step.
* **Timestamp domain**: whether a target's timestamps are uptime or UTC is not
  yet part of the protocol; blip guesses it from the values.
