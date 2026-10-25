sonde
=====

sonde is a C++17 library that traces values and log records on an embedded
target or in a unit test and encodes them as blip messages. It hands them over
in **batches**: the firmware sends them, e.g. over UDP, TCP or UART to an
outpost, or a **sink** writes them to a recording file that blip opens.

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

Cross-compile the library and the bare-metal example for a Cortex-M::

   cmake -S . -B build-m0 -DCMAKE_TOOLCHAIN_FILE=cmake/arm-none-eabi.cmake -DSONDE_CPU=cortex-m0
   cmake --build build-m0

First trace
-----------

.. code-block:: cpp

   #include <sonde/sonde.hpp>

   static sonde::Storage<512, 2> storage;              // 2 batches of 512 bytes
   static sonde::Tracer tracer{storage, &uptime_ns};   // your clock, in ns

   tracer.metadata("motor/speed", "rpm");
   tracer.value("motor/speed", 1500u);
   tracer.value("cpu_load", 12.5f);
   tracer.value("sensor/temperature", sonde::null);    // failed measurement, a gap in the plot
   tracer.log(sonde::Level::Info, "motor", "started");
   tracer.logf(sonde::Level::Warn, "motor", "overcurrent: %d mA", current_ma);

Each call encodes one message and returns a ``sonde::Status``: ``Ok``,
``Full`` (no free batch, dropped), ``TooLarge`` (does not fit into an empty
batch, dropped), ``SinkError`` or ``Invalid`` (e.g. a record with fewer fields
than announced; nothing written). ``tracer.dropped()`` counts the dropped
messages.

.. list-table:: Calls and the messages they send
   :header-rows: 1
   :widths: 45 55

   * - Call
     - Message
   * - ``value(path, value)``, ``value(timestamp, path, value)``
     - ``Value``, timestamped by the clock or explicitly
   * - ``metadata(path, unit)``
     - ``ValueMetadata``
   * - ``log(level, component, message)``, ``log(timestamp, …)``, ``logf(…)``
     - ``Log``; an empty component is left out
   * - ``commit()``
     - none: makes the open batch ready to send

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
or elements is given up front, as CBOR encodes it before them:

.. code-block:: cpp

   float acc[3] = {0.5f, -1.0f, 9.81f};
   tracer.value("imu/acc", acc);                       // [0.5, -1.0, 9.81]

   tracer.value("imu", sonde::record(2, [&](sonde::Record& r) {
       r.field("acc", acc);
       r.field("valid", true);
   }));                                                // {"acc": [...], "valid": true}

   tracer.value("adc/scaled", sonde::array(64, [&](sonde::Array& a) {
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

   tracer.value("motor", motor);

A message that does not fit into the open batch is encoded again into the
next one, so fill and ``encode`` functions should have no side effects.

Timestamps and clocks
---------------------

A timestamp is nanoseconds in the time of the tracer's clock: the uptime of the
target, or UTC since 1970, e.g. on a desktop. The clock is a plain function
``std::uint64_t (*)()`` called in the tracing context, so it must be safe
there, e.g. in an ISR. Hosted builds default to ``sonde::system_clock_ns``
(UTC); on targets, pass your own. Without a clock, messages traced without a
timestamp get 0.

blip shows uptimes as time since boot, see :doc:`concepts`.

Sending from firmware
---------------------

Each context that traces has its **own tracer** with its own storage, e.g. one
for the control task and one for an ISR. A tracer is never shared between
contexts; sonde has no global tracer.

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

   The batches of a tracer.

Trace in the control task, send one UDP datagram per batch in the network
task:

.. code-block:: cpp

   static sonde::Storage<1400, 4> control_storage;     // a batch fits the Ethernet MTU
   static sonde::Tracer control_traces{control_storage, &uptime_ns};

   void control_task() {                               // producer
       for (;;) {
           wait_for_cycle();
           control_traces.value("motor/speed", read_speed());
           control_traces.value("motor/current", read_current());
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
   static sonde::Tracer adc_traces{adc_storage, &uptime_ns};

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
* One consumer may serve several tracers.
* Batch size and count are yours: e.g. the MTU for UDP, more batches for bursts.
  Add framing where the link needs it, e.g. COBS on a UART.
* A batch only ever holds whole messages, a CBOR sequence, so each datagram or
  write decodes on its own. Receive it with the outpost's ``tcp`` or ``udp``
  protocol, see :doc:`outpost`.

The constructors are ``constexpr``: a static tracer is ready before any code
runs, e.g. for an ISR that fires before the static constructors.

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
   sonde::Tracer tracer{storage, file};                // UTC from the system clock

   tracer.value("solver/residual", residual);
   // ...
   tracer.commit();                                    // before the file closes

A unit test can write a recording per test, to open in blip when it fails; a
fixed clock keeps the timestamps reproducible:

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
       sonde::Tracer tracer{storage, file, &test_clock};

       SpeedController controller{tracer};
       for (steps = 0; steps < 500; ++steps) {
           controller.step(1500.0);
       }
       tracer.commit();

       CHECK(controller.speed() == Catch::Approx(1500.0).margin(5.0));
   }

The example ``sonde/examples/desktop.cpp`` traces a damped oscillation to
``oscillation.blip``.

More
----

``sonde/README.md`` in the repository has the full API, the design decisions
and the tests; :doc:`/architecture/sonde` describes how it works inside.
