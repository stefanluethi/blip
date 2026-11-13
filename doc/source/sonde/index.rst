sonde
=====

sonde is a C++17 library that traces values and log records on an embedded
target or in a unit test and encodes them as blip messages. It hands them over
in **batches**: the firmware sends them, e.g. over UDP, TCP or UART to an
outpost, or a **sink** writes them to a recording file that blip opens.

.. admonition:: Before you begin

   A C++17 compiler and CMake 3.16 or newer (see :doc:`/start/installation`), and an
   outpost to receive what the firmware sends (``blip-outpost tcp`` or ``udp``,
   see :doc:`/reference/outpost-cli`) or blip to open the recording files of unit
   tests.

* No heap allocation, works with ``-fno-exceptions -fno-rtti``, no iostreams;
  you provide the storage.
* The context that traces and the one that sends may differ -- two tasks, an
  ISR and a task -- without locks.
* Runs in unit tests and desktop apps too, with file output and a wall clock.

Actions are not supported yet.

Adding it to a project
----------------------

.. code-block:: cmake

   add_subdirectory(sonde)
   target_link_libraries(firmware PRIVATE sonde::sonde)

sonde fetches `tinycbor <https://github.com/intel/tinycbor>`_ v7.0 with
CMake's FetchContent. Firmware that vendors tinycbor sets
``SONDE_FETCH_TINYCBOR=OFF`` and provides a ``tinycbor`` target.

.. list-table:: CMake options
   :header-rows: 1
   :widths: 30 25 45

   * - Option
     - Default
     - Meaning
   * - ``SONDE_HOSTED``
     - ``ON`` unless cross-compiling
     - file sinks and the system clock
   * - ``SONDE_FETCH_TINYCBOR``
     - ``ON``
     - fetch tinycbor; ``OFF`` uses a ``tinycbor`` target of the project
   * - ``SONDE_BUILD_TESTS``
     - ``ON`` if top-level and hosted
     - fetch Catch2 v3 and build the tests
   * - ``SONDE_BUILD_EXAMPLES``
     - ``ON`` if top-level
     - build the examples
   * - ``SONDE_WERROR``
     - ``OFF``
     - treat the warnings of sonde as errors
   * - ``SONDE_LOGF_MAX``
     - ``128``
     - bytes of a ``logf`` message
   * - ``SONDE_PATH_MAX``
     - ``96``
     - bytes of the full path of a message of a ``Tracer``

Cross-compile the library and the bare-metal example for a Cortex-M::

   cmake -S . -B build-m0 -DCMAKE_TOOLCHAIN_FILE=cmake/arm-none-eabi.cmake -DSONDE_CPU=cortex-m0
   cmake --build build-m0

First trace
-----------

A **queue** holds the batches of one context that traces. **Tracers** on it are
the named parts of the code, as loggers: the traces of a tracer are below its
name, its log records at its name.

.. code-block:: cpp

   #include <sonde/sonde.hpp>

   static sonde::Storage<512, 2> storage;              // 2 batches of 512 bytes
   static sonde::Queue queue{storage, &uptime_ns,      // your clock, in ns
                             sonde::Hello{"rig-7", sonde::TimeBase::Boot}};
   static sonde::Tracer motor{queue, "motor"};
   static sonde::Tracer left{motor, "left"};           // motor/left

   motor.unit("speed", "rpm");
   motor.value("speed", 1500u);                        // the trace motor/speed
   motor.value("temperature", sonde::null);            // failed measurement, a gap in the plot
   motor.info("started");                              // a log record at motor
   motor.warnf("overcurrent: %d mA", current_ma);
   left.value("current", 1.2f);                        // motor/left/current

Each call encodes one message and returns a ``sonde::Status``: ``Ok``,
``Full`` (no free batch, dropped), ``TooLarge`` (does not fit into an empty
batch, or its full path is longer than ``SONDE_PATH_MAX``; dropped),
``SinkError`` or ``Invalid`` (e.g. a record with fewer fields than announced;
nothing written). ``queue.dropped()`` counts the dropped messages.

.. list-table:: Calls of a tracer and the messages they send
   :header-rows: 1
   :widths: 45 55

   * - Call
     - Message
   * - ``value(path, value)``, ``value(timestamp, path, value)``
     - ``Sample`` of the trace ``path`` below the tracer, timestamped by the
       clock or explicitly
   * - ``samples(path, values, timestamp, Period{ns})``,
       ``samples(path, values, timestamps)``
     - ``Samples``, a block of the trace ``path`` below the tracer, see
       `Blocks of samples`_
   * - ``unit(path, unit)``, ``metadata(path, Metadata{…})``
     - ``TraceMetadata``, see `Metadata`_
   * - ``log(level, message)``, ``log(timestamp, level, message)``,
       ``logf(level, format, …)``, ``debug`` … ``error(message)``,
       ``debugf`` … ``errorf(format, …)``
     - ``Log`` at the name of the tracer; without a name,
       of the target as a whole

A tracer is a few pointers: any number of them may share a queue, and a child,
``Tracer{parent, "left"}``, sends below its parent.
``Tracer{queue}`` without a name sends the paths as they are. A tracer keeps
a view of its name, so its string has to outlive it, as
string literals do.

The queue has the same calls with the full path, and log records with their
path, e.g. for a gateway that passes on what other nodes send:

.. code-block:: cpp

   queue.value(sonde::Timestamp{t}, "node3/motor/speed", speed);
   queue.log(sonde::Timestamp{t}, "log/node3", sonde::Level::Warn, "hot");
   queue.unit("node3/motor/speed", "rpm");

It also has ``commit()``, which makes the open batch ready to send, and the
calls of the consumer, see `Sending from firmware`_.

Every queue sends a ``Hello`` before its first message, see `Hello`_.

.. _sonde-values:

Values
------

``value()`` takes any of these:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - In blip
     - C++
   * - ``null``
     - ``nullptr``, ``sonde::null``, an empty ``std::optional<T>``
   * - integer
     - any integer type except ``char`` and ``bool``
   * - float
     - ``float``, ``double``, encoded in the shortest width that keeps the
       value
   * - bool
     - ``bool``
   * - string
     - ``std::string_view``, ``const char*``, string literals
   * - array
     - ``T[N]``, ``std::array<T, N>``, ``sonde::Span<T>``,
       ``sonde::array(n, fill)``
   * - record
     - ``sonde::record(n, fill)``, your types with an ``encode`` function

Records and computed arrays are built by a fill function; the number of fields
or elements is given up front, as CBOR encodes it before them. Each field of a
record is a trace, its path joined with ``.``. With the tracers ``imu`` and
``adc``:

.. code-block:: cpp

   float acc[3] = {0.5f, -1.0f, 9.81f};
   imu.value("acc", acc);                              // [0.5, -1.0, 9.81]

   imu.value("state", sonde::record(2, [&](sonde::Record& r) {
       r.field("acc", acc);
       r.field("valid", true);
   }));                                                // imu/state.acc, imu/state.valid

   adc.value("scaled", sonde::array(64, [&](sonde::Array& a) {
       for (auto s : samples) a.add(s * 0.1f);
   }));

Your own types become values with an ``encode`` function next to the type:

.. code-block:: cpp

   struct Motor {
       std::uint32_t speed_rpm;
       bool enabled;
       const char* state;
   };

   void encode(sonde::Encoder& encoder, const Motor& motor) {
       encoder.record(3, [&](sonde::Record& r) {
           r.field("speed_rpm", motor.speed_rpm);
           r.field("enabled", motor.enabled);
           r.field("state", motor.state);
       });
   }

   drive.value("motor", motor);                        // drive/motor.speed_rpm, …

A message that does not fit into the open batch is encoded again into the
next one, so fill and ``encode`` functions should have no side effects.

Metadata
--------

``unit()`` tells blip the unit of a trace. ``metadata()`` sends the unit, a
description, labels for integer values and the range the values are expected in,
as one ``TraceMetadata``; what is empty is left out. Give the labels in
ascending order of their value:

.. code-block:: cpp

   static const sonde::Label modes[] = {{0, "idle"}, {1, "ramp"}, {2, "run"}};
   motor.metadata("mode", {{}, "Operating mode", {modes, 3}});
   motor.metadata("speed", {"rpm", "Shaft speed", {}, sonde::Range{0, 3000}});

blip shows the labels in place of the numbers, the description on hover and in
tooltips, and fits plots to the range. Send the metadata of a trace in one call:
a newer one replaces the earlier.

Blocks of samples
-----------------

``samples()`` sends many samples of one trace as one message, much smaller than
a ``value()`` for each. ``values`` is a C array, a ``std::array``, a
``sonde::Span`` or ``sonde::array(n, fill)``. For samples at a fixed period give
the timestamp of the first one and the period; sample ``i`` is at the timestamp
plus ``i`` times the period. For irregular ones give a timestamp for each. The
clock is not read for a block.

.. code-block:: cpp

   static std::uint16_t dma_buffer[64];

   adc.samples("raw", dma_buffer, sonde::Timestamp{block_start_ns}, sonde::Period{250'000});

   const float peaks[2] = {1.5f, 2.5f};
   const sonde::Timestamp at[2] = {{4'000'000}, {4'500'000}};
   adc.samples("peak", peaks, at);             // Invalid if the counts differ

A block that is larger than a batch is ``TooLarge``: split it.

Hello
-----

A queue sends a ``Hello`` once, before the first message of any kind, in the same
batch: the version of the protocol (1), the name of the target, the clock of its
timestamps and ``sonde 0.2.0`` as the sender. Give the name and the time base to the
queue; a queue without a ``Hello`` sends the version and the sender only, and
blip guesses the time base.

.. code-block:: cpp

   sonde::Queue queue{storage, &uptime_ns, sonde::Hello{"rig-7", sonde::TimeBase::Boot}};

The queues of one link, e.g. a control task and an ISR on one UART, each send a
``Hello``: give them the same one. It is not repeated, and cannot be turned off.

Timestamps and clocks
---------------------

A timestamp is nanoseconds in the time of the queue's clock: the uptime of the
target, or UTC since 1970, e.g. on a desktop. The clock is a plain function
``std::uint64_t (*)()`` called in the tracing context, so it must be safe
there, e.g. in an ISR. Hosted builds default to ``sonde::system_clock_ns``
(UTC); on targets, pass your own. Without a clock, messages traced without a
timestamp get 0.

The time base of the ``Hello`` tells blip which: ``TimeBase::Boot`` for an
uptime, ``TimeBase::Epoch`` for UTC. blip shows uptimes as time since boot, see
:doc:`/overview/concepts`; with ``TimeBase::Unknown``, the default, it guesses
from the timestamps, which fails for a UTC clock that is not set yet. Give the time base
whenever the clock is known. When the
target restarts and its uptime starts over, the outpost shifts the new
timestamps to follow the ones before.

Sending from firmware
---------------------

Each context that traces has its **own queue** with its own storage, e.g. one
for the control task and one for an ISR. A queue is never shared between
contexts, and the tracers on a queue are used in its context only; sonde has no
global queue.

The context that traces (the **producer**) encodes into the open batch. When a
message does not fit, or on ``commit()``, the batch becomes ready. The context
that sends (the **consumer**) takes the oldest ready batch with ``take()``,
sends it and gives it back with ``release()``. The producer never blocks: when
no batch is free it drops the message.

.. figure:: /diagrams/sonde-batches.svg
   :target: ../_images/sonde-batches.svg
   :class: diagram
   :alt: The producer encodes into the open batch; commit makes it ready; the
         consumer takes ready batches, sends them and releases them back to
         free.

   The batches of a queue.

Trace in the control task, send one UDP datagram per batch in the network
task:

.. code-block:: cpp

   static sonde::Storage<1400, 4> control_storage;     // a batch fits the Ethernet MTU
   static sonde::Queue control_traces{control_storage, &uptime_ns};
   static sonde::Tracer motor{control_traces, "motor"};

   void control_task() {                               // producer
       for (;;) {
           wait_for_cycle();
           motor.value("speed", read_speed());
           motor.value("current", read_current());
           control_traces.commit();                    // one batch per cycle at most
       }
   }

   void network_task() {                               // consumer
       for (;;) {
           while (auto batch = control_traces.take()) {
               udp_send(batch.data, batch.size);
               control_traces.release();
           }
           sleep_ms(5);
       }
   }

Trace in an ISR, send by DMA over a UART:

.. code-block:: cpp

   static sonde::Storage<256, 3> adc_storage;
   static sonde::Queue adc_traces{adc_storage, &uptime_ns};

   void ADC_IRQHandler() {                             // producer
       adc_traces.value("adc/raw", ADC1->DR);
   }

   void comm_task() {                                  // consumer
       for (;;) {
           adc_traces.request_commit();                // the ISR commits at its next sample
           if (!uart_dma_busy()) {
               if (auto batch = adc_traces.take()) {
                   uart_dma_send_cobs(batch.data, batch.size);
               }
           }
           sleep_ms(10);
       }
   }

   void DMA_TxComplete_IRQHandler() {
       adc_traces.release();
   }

* The consumer cannot close the open batch, as the producer may be encoding
  into it; ``request_commit()`` asks the producer to commit at its next message
  or ``commit()``. Producers that trace rarely call ``commit()`` themselves.
* One consumer may serve several queues.
* Batch size and count are yours: e.g. the MTU for UDP, more batches for bursts.
  Add framing where the link needs it, e.g. COBS on a UART.
* A batch only ever holds whole messages, a CBOR sequence, so each datagram or
  write decodes on its own. Receive it with ``blip-outpost tcp`` or ``udp``,
  see :doc:`/outpost/index`.

The constructors are ``constexpr``: a static queue and its static tracers are
ready before any code runs, e.g. for an ISR that fires before the static
constructors.

Desktop apps and unit tests
---------------------------

With a **sink**, producer and consumer are one context: each ready batch goes
to the sink right away. ``sonde/file_sink.hpp`` (hosted builds) has two:

* ``FileSink`` writes a recording file that blip opens and the outpost
  replays. Missing directories of its path are created.
* ``MemorySink`` keeps the bytes in memory, to check them in a test.

.. code-block:: cpp

   #include <sonde/file_sink.hpp>

   sonde::FileSink file{"run.blip"};
   sonde::Storage<4096, 1> storage;
   sonde::Queue queue{storage, file};                  // UTC from the system clock
   sonde::Tracer solver{queue, "solver"};

   solver.value("residual", residual);                 // solver/residual
   // ...
   queue.commit();                                     // before the file closes

A unit test can write a recording per test, to open in blip when it fails; a
fixed clock keeps the timestamps reproducible. The code under test takes a
``sonde::Tracer&``:

.. code-block:: cpp

   #include <catch2/catch_test_macros.hpp>
   #include <sonde/file_sink.hpp>

   namespace {
   std::uint64_t steps = 0;
   std::uint64_t test_clock() { return steps * 1'000'000; }   // 1 ms per step
   }

   TEST_CASE("speed controller settles") {
       sonde::FileSink file{"traces/speed_controller_settles.blip"};
       sonde::Storage<4096, 1> storage;
       sonde::Queue queue{storage, file, &test_clock};
       sonde::Tracer tracer{queue, "controller"};

       SpeedController controller{tracer};
       for (steps = 0; steps < 500; ++steps) {
           controller.step(1500.0);
       }
       queue.commit();

       CHECK(controller.speed() == Catch::Approx(1500.0).margin(5.0));
   }

The example ``sonde/examples/desktop.cpp`` traces a damped oscillation to
``oscillation.blip``.

More
----

``sonde/README.md`` in the repository has the design decisions and the tests;
:doc:`/sonde/internals` describes how it works inside.

Next steps
----------

* :doc:`/reference/sonde-api` -- all of the API, from the headers.
* :doc:`/reference/outpost-cli` -- pass the batches of the firmware on to blip with ``tcp`` or ``udp``.
* :doc:`/help/troubleshooting` -- when the times or the values look wrong.

.. toctree::
   :hidden:

   internals
