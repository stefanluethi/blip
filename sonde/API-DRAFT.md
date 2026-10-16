# sonde: API and usage draft

sonde is a C++17 library that collects traces on an embedded target or in a unit test and
encodes them as blip messages. It hands them over in batches, which the firmware sends
(e.g. over UDP, TCP or UART) or a sink writes to a file that blip opens.

```
firmware / unit test
  └─ sonde ──(CBOR sequence over TCP / UDP / UART)──> outpost ──(WebSocket)──> blip
           └─(recording file)────────────────────────────────────────────────> blip
```

Status: draft for discussion, nothing implemented yet.

## Goals

- **C++17**, built with **CMake**.
- **Runs on microcontrollers**: no heap allocation, works with `-fno-exceptions -fno-rtti`,
  no iostreams. The user provides the storage.
- **Safe across contexts**: the context that traces and the one that sends the batches
  may differ (two tasks, an ISR and a task), without locks.
- **Runs in unit tests and desktop apps**: hosted extras (file output, wall clock) are
  enabled when the platform has a file system.
- **API close to `proto`**: one call per `Message` variant, with the same fields in the
  same order.
- **Batches**: several messages go into one batch as a CBOR sequence (RFC 8742), i.e.
  CBOR items one after another. A batch only ever holds complete messages.
- **Encoding with [tinycbor](https://github.com/intel/tinycbor)**.
- **Output to a buffer or a file**: the firmware takes the batches and sends them, or a
  sink writes them to a file.
- **Tests with Catch2**.

## Concepts

| Concept   | Meaning |
|-----------|---------|
| `Tracer`  | Encodes messages into batches. One tracer per context that traces. |
| batch     | A slot of the storage holding complete messages, a CBOR sequence ready to send or write. |
| `Storage` | The batches of a tracer, provided by the user: `Batches` slots of `BatchBytes` bytes each. |
| producer  | The context that traces with a tracer: a task, a thread or an ISR. |
| consumer  | The context that takes the finished batches and sends them. |
| `Sink`    | Where batches go when producer and consumer are one context, e.g. a file in a unit test. |
| encodable | A C++ value that maps to `proto::Data`: scalars, strings, arrays, records, user types. |

### Mapping to `proto`

| `proto::Message`      | sonde call |
|-----------------------|------------|
| `Value { timestamp, path, value }` | `tracer.value(path, value)` / `tracer.value(timestamp, path, value)` |
| `ValueMetadata { path, unit }`     | `tracer.metadata(path, unit)` |
| `Log { timestamp, level, component, message }` | `tracer.log(level, component, message)` / `tracer.log(timestamp, level, component, message)` |
| `Actions`, `ActionResult`, `Request::Invoke` | later, see [Actions](#later-actions) |

| `proto::Data` | C++ |
|---------------|-----|
| `Null`        | `nullptr`, `sonde::null`, an empty `std::optional<T>` |
| `Uint` / `Int`| any integer type except `char` and `bool` (the sign decides the CBOR major type, as in `proto`) |
| `Float`       | `float`, `double`, encoded in the shortest width (16, 32 or 64 bit) that keeps the value |
| `Bool`        | `bool` |
| `String`      | `std::string_view`, `const char*`, string literals |
| `Array`       | `T[N]`, `std::array<T, N>`, `sonde::Span<T>`, `sonde::array(n, fill)` |
| `Record`      | `sonde::record(n, fill)`, user types with an `encode` function |

## Why sonde builds the batches

Batching could also be left to the firmware: sonde would encode single messages, and the
firmware would collect them into packets. sonde builds the batches itself, because:

- **A CBOR sequence costs nothing.** It is the messages one after another, no header, no
  length, no separator. Encoding the next message right behind the previous one *is* the
  batch; there is nothing to bundle afterwards.
- **No copy and no second buffer.** The message is encoded directly into the bytes that
  get sent. Collecting in the firmware means encoding into a scratch buffer, copying into
  a packet buffer and tracking what fits, in every project again.
- **Whole messages per batch.** Only the encoder knows where a message ends and whether
  the next one fits; sonde guarantees that a batch never ends in half a message, so each
  UDP datagram or file write decodes on its own.
- **The batch is the handover between contexts.** Handing over whole batches through a
  lock-free queue is what makes tracing in one context and sending in another safe (see
  [Contexts](#contexts)); single messages would need a queue of their own.

What stays with the firmware is *when* and *how* to send: it chooses the batch size (e.g.
the MTU), commits batches when it wants (e.g. once per control cycle), takes them when the
link is ready, and adds transport framing where the link needs it, e.g. COBS on a UART so
the receiver finds the next message after a lost byte. A firmware that wants one message
per packet commits after each message.

## Contexts

Each context that traces has its **own tracer** with its own storage, e.g. one for the
control task, one for an ISR and one for the communication task. A tracer is never shared
between producers; sonde has no global tracer, since a global one would be used from
several contexts.

Within a tracer, the storage is a **single-producer single-consumer queue of batches**:

```
            producer                                  consumer
        (task, thread, ISR)                     (send task, DMA ISR)

  value() / log() ──> [ open ] ──commit()──> [ ready ][ ready ] ──take()──> send
                         ▲                                                  │
                         └──────────────────── [ free ] <──release()────────┘
```

- The **producer** encodes into the open batch. When a message does not fit, or on
  `commit()`, the open batch becomes ready and the next free one opens.
- The **consumer** takes the oldest ready batch with `take()`, sends it and gives it back
  with `release()`, e.g. from the DMA-complete interrupt.
- Producer and consumer only share two indices, `std::atomic<std::uint32_t>`, read with
  acquire and written with release; each index is written by one side only. That needs no
  locks and no compare-and-swap, so it also works on Cortex-M0 (no `LDREX`/`STREX`), from
  ISRs, and on multicore MCUs. sonde `static_assert`s that the atomics are always
  lock-free.
- The producer never blocks: when no batch is free it drops the message, returns
  `Status::Full` and counts it in `dropped()`. Tracing does not change the timing of the
  traced code because the link is slow.
- The consumer cannot close the open batch, since the producer may be encoding into it.
  It can ask with `request_commit()`, and the producer commits at its next message or
  `commit()`. Producers that trace rarely call `commit()` themselves, e.g. at the end of
  their cycle.
- One consumer may serve several tracers, e.g. a communication task that takes the
  batches of all tracers in turn.

With a **sink**, producer and consumer are one context: a ready batch goes to the sink
right away, in the producer's context. That is for desktop apps and unit tests, writing to
a file; one or two batches are enough there.

| Use                                | Storage            | Who sends |
|------------------------------------|--------------------|-----------|
| MCU, trace in a task, send in another | `Batches >= 2`  | consumer: `take()` / `release()` |
| MCU, trace in an ISR, send in a task  | `Batches >= 2`  | consumer: `take()` / `release()` |
| desktop app, unit test, to a file     | `Batches >= 1`  | sink, in the producer's context |

## API

All of it is in namespace `sonde`. Headers:

| Header                  | Contents | Availability |
|-------------------------|----------|--------------|
| `sonde/sonde.hpp`       | `Tracer`, `Storage`, `Sink`, the types and the encoding of values | everywhere |
| `sonde/file_sink.hpp`   | `FileSink`, `MemorySink` | hosted builds (`SONDE_HOSTED`) |

### Types

```cpp
namespace sonde {

/// Nanoseconds in the time domain of the clock: the uptime of a target or UTC (since
/// the Unix epoch). See "Timestamps".
struct Timestamp {
    std::uint64_t ns;
};

/// Returns the current time in nanoseconds. Called from the producer's context, so it
/// has to be safe there, e.g. in an ISR.
using Clock = std::uint64_t (*)();

/// The default clock: UTC from the system clock in hosted builds, none (nullptr) on
/// targets, which pass their own.
Clock default_clock();

/// The levels of log records as blip knows them (proto: LogRecord::level).
enum class Level : std::uint8_t { Trace = 0, Debug = 1, Info = 2, Warn = 3, Error = 4 };

enum class Status : std::uint8_t {
    Ok,
    /// No batch is free, the consumer has not released them yet. The message was dropped.
    Full,
    /// The message does not fit even into an empty batch, it was dropped.
    TooLarge,
    /// The sink failed to take a batch; the batch is dropped.
    SinkError,
    /// The message could not be encoded, e.g. a record with fewer fields than announced.
    /// The batch is unchanged.
    Invalid,
};

/// A view of elements, since C++17 has no std::span.
template <typename T>
struct Span {
    const T* data;
    std::size_t size;
};

inline constexpr std::nullptr_t null = nullptr;

/// A ready batch, as taken by the consumer.
struct Batch {
    const std::uint8_t* data;  // nullptr if there is none
    std::size_t size;
    explicit operator bool() const { return data != nullptr; }
};

}  // namespace sonde
```

### Storage

```cpp
namespace sonde {

/// The batches of one tracer: Batches slots of BatchBytes bytes, plus their sizes.
/// Static or on the stack, owned by the user, outlives the tracer.
template <std::size_t BatchBytes, std::size_t Batches = 2>
class Storage;

}  // namespace sonde
```

### Sink

```cpp
namespace sonde {

/// Takes the batches of a tracer whose producer and consumer are one context.
class Sink {
public:
    /// Takes a batch of complete messages, a CBOR sequence. Returns false on failure.
    virtual bool write(const std::uint8_t* data, std::size_t size) = 0;

protected:
    ~Sink() = default;
};

}  // namespace sonde
```

### Tracer

```cpp
namespace sonde {

class Tracer {
public:
    /// Producer and consumer: the consumer takes the batches.
    template <std::size_t BatchBytes, std::size_t Batches>
    explicit Tracer(Storage<BatchBytes, Batches>& storage, Clock clock = default_clock());

    /// One context: each ready batch goes to the sink right away.
    template <std::size_t BatchBytes, std::size_t Batches>
    Tracer(Storage<BatchBytes, Batches>& storage, Sink& sink, Clock clock = default_clock());

    Tracer(const Tracer&) = delete;
    Tracer& operator=(const Tracer&) = delete;

    // --- Producer: only from the context that traces --------------------------------

    // proto: Message::Value, timestamped by the clock or explicitly
    template <typename T>
    Status value(std::string_view path, const T& value);
    template <typename T>
    Status value(Timestamp timestamp, std::string_view path, const T& value);

    // proto: Message::ValueMetadata
    Status metadata(std::string_view path, std::string_view unit);

    // proto: Message::Log; an empty component is left out on the wire
    Status log(Level level, std::string_view component, std::string_view message);
    Status log(Timestamp timestamp, Level level, std::string_view component,
               std::string_view message);
    /// printf-like, formatted into a buffer of SONDE_LOGF_MAX bytes on the stack,
    /// longer messages are cut off.
    Status logf(Level level, std::string_view component, const char* format, ...);

    /// Makes the open batch ready, if it holds messages. With a sink, writes it.
    Status commit();

    // --- Consumer: only from the context that sends ----------------------------------

    /// The oldest ready batch, or none. Stays valid until release().
    Batch take();
    /// Gives the batch of the last take() back to the producer.
    void release();
    /// Asks the producer to commit the open batch at its next message or commit().
    void request_commit();

    // --- Either context ---------------------------------------------------------------

    /// Messages dropped since construction, because no batch was free, a message was too
    /// large or the sink failed.
    std::uint32_t dropped() const;
};

}  // namespace sonde
```

Messages are atomic: each message is encoded by a fresh tinycbor encoder into the free part
of the open batch. If it does not fit, the batch keeps its size; the tracer commits it and
encodes the message into the next free batch, or drops it if there is none.

### Values

`value()` takes any encodable. Besides the scalars, strings and arrays of the
[mapping](#mapping-to-proto), records and arrays of computed elements are built with a
fill function, which a template parameter takes (no `std::function`, no heap):

```cpp
namespace sonde {

/// Encodes one Data item.
class Encoder {
public:
    template <typename T>
    void encode(const T& value);
    void null();
    /// A record of `fields` fields, filled by fill(Record&).
    template <typename F>
    void record(std::size_t fields, F&& fill);
    /// An array of `elements` elements, filled by fill(Array&).
    template <typename F>
    void array(std::size_t elements, F&& fill);
};

class Record {
public:
    template <typename T>
    void field(std::string_view name, const T& value);
};

class Array {
public:
    template <typename T>
    void add(const T& value);
};

/// Encodables of a record or an array built by a fill function, e.g. for value().
template <typename F>
auto record(std::size_t fields, F&& fill);  // fill(Record&)
template <typename F>
auto array(std::size_t elements, F&& fill);  // fill(Array&)

}  // namespace sonde
```

The number of fields or elements is given up front, since CBOR encodes it before them;
tinycbor checks it when the record or array is closed, and the message then fails with
`Status::Invalid`.

User types become encodable with an `encode` function next to the type, found by
argument-dependent lookup:

```cpp
void encode(sonde::Encoder& encoder, const Motor& motor);
```

### File sinks (hosted builds)

```cpp
namespace sonde {

/// Writes a blip recording: the header {"format": "blip recording", "version": 1} and
/// then the batches, uncompressed. blip opens the file directly, the outpost replays it.
class FileSink final : public Sink {
public:
    explicit FileSink(const char* path);
    ~FileSink();
    bool is_open() const;
    bool write(const std::uint8_t* data, std::size_t size) override;
};

/// Keeps the batches in memory, e.g. to check them in a unit test.
class MemorySink final : public Sink {
public:
    bool write(const std::uint8_t* data, std::size_t size) override;
    const std::vector<std::uint8_t>& bytes() const;
    void clear();
};

}  // namespace sonde
```

## Timestamps

A timestamp is nanoseconds in the time domain of the tracer's clock: the **uptime** of a
target, or **UTC** since the Unix epoch, e.g. on a desktop or a target with a synchronized
clock. sonde does not convert between them.

`proto` documents `Value::timestamp` as UTC so far. It needs an update to say which
domain a connection's timestamps are in (e.g. in a message at the start of a connection),
so blip can show uptimes as such. Until then uptimes show as times shortly after 1970,
with correct distances between them.

## Usage

### Values, metadata and logs

```cpp
#include <sonde/sonde.hpp>

static sonde::Storage<512, 2> storage;
static sonde::Tracer tracer{storage, &uptime_ns};

tracer.metadata("motor/speed", "rpm");
tracer.value("motor/speed", 1500u);
tracer.value("cpu_load", 12.5f);
tracer.value("sensor/temperature", sonde::null);  // failed measurement, a gap in the plot
tracer.value(sonde::Timestamp{t_ns}, "adc/raw", raw);
tracer.log(sonde::Level::Info, "motor", "started");
tracer.logf(sonde::Level::Warn, "motor", "overcurrent: %d mA", current_ma);
```

### Arrays, records and user types

```cpp
float acc[3] = {0.5f, -1.0f, 9.81f};
tracer.value("imu/acc", acc);  // [0.5, -1.0, 9.81]

tracer.value("imu", sonde::record(2, [&](sonde::Record& r) {
    r.field("acc", acc);
    r.field("valid", true);
}));  // {"acc": [0.5, -1.0, 9.81], "valid": true}

std::uint16_t samples[64];
tracer.value("adc/scaled", sonde::array(64, [&](sonde::Array& a) {
    for (auto s : samples) a.add(s * 0.1f);
}));
```

```cpp
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

tracer.value("motor", motor);  // {"speed_rpm": 1500, "enabled": true, "state": "running"}
```

### Microcontroller: trace in the control task, send in the network task

One UDP datagram per batch, the batch size fits the Ethernet MTU:

```cpp
static sonde::Storage<1400, 4> control_storage;
static sonde::Tracer control_traces{control_storage, &uptime_ns};

void control_task() {  // producer
    for (;;) {
        wait_for_cycle();
        control_traces.value("motor/speed", read_speed());
        control_traces.value("motor/current", read_current());
        control_traces.value("pid", pid_state);
        control_traces.commit();  // one batch per cycle at most
    }
}

void network_task() {  // consumer
    for (;;) {
        while (auto batch = control_traces.take()) {
            udp_send(batch.data, batch.size);
            control_traces.release();
        }
        sleep_ms(5);
    }
}
```

### Microcontroller: trace in an ISR, send by DMA over UART

The ISR has its own tracer. The DMA-complete interrupt releases the batch:

```cpp
static sonde::Storage<256, 3> adc_storage;
static sonde::Tracer adc_traces{adc_storage, &uptime_ns};

void ADC_IRQHandler() {  // producer
    adc_traces.value("adc/raw", ADC1->DR);
}

void comm_task() {  // consumer
    for (;;) {
        adc_traces.request_commit();  // the ISR commits at its next sample
        if (!uart_dma_busy()) {
            if (auto batch = adc_traces.take()) {
                uart_dma_send_cobs(batch.data, batch.size);  // framing on the UART
            }
        }
        sleep_ms(10);
    }
}

void DMA_TxComplete_IRQHandler() {
    adc_traces.release();
}
```

### Desktop app, to a file

```cpp
#include <sonde/file_sink.hpp>

sonde::FileSink file{"run.blip"};
sonde::Storage<4096, 1> storage;
sonde::Tracer tracer{storage, file};  // UTC from the system clock

tracer.value("solver/residual", residual);
// ...
tracer.commit();  // before the file closes; then open run.blip in blip
```

### Unit test with Catch2

The code under test takes a `Tracer&`. Each test writes its own recording, to open in blip
when a test fails, and a fixed clock keeps the timestamps reproducible:

```cpp
#include <catch2/catch_test_macros.hpp>
#include <sonde/file_sink.hpp>

namespace {
std::uint64_t steps = 0;
std::uint64_t test_clock() { return steps * 1'000'000; }  // 1 ms per step, an "uptime"
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
```

`MemorySink` checks what the code traced:

```cpp
TEST_CASE("controller traces its state each step") {
    sonde::MemorySink memory;
    sonde::Storage<512, 1> storage;
    sonde::Tracer tracer{storage, memory, &test_clock};

    SpeedController controller{tracer};
    controller.step(1500.0);
    tracer.commit();

    CHECK(count_values(memory.bytes(), "controller/error") == 1);  // a test helper
}
```

## Build

```
sonde/
  CMakeLists.txt
  include/sonde/       sonde.hpp, file_sink.hpp
  src/                 tracer.cpp, encoder.cpp, file_sink.cpp
  tests/               Catch2 tests
  examples/            desktop and a bare-metal sketch
```

| CMake option           | Default | Meaning |
|------------------------|---------|---------|
| `SONDE_HOSTED`         | `ON` unless cross-compiling | file sinks, system clock |
| `SONDE_FETCH_TINYCBOR` | `ON`    | fetch tinycbor with FetchContent; `OFF` uses a `tinycbor` target the project provides (vendored in firmware) |
| `SONDE_BUILD_TESTS`    | `ON` if top-level project | fetch Catch2 v3 and build the tests |
| `SONDE_LOGF_MAX`       | `128`   | bytes of a `logf` message |

tinycbor has no CMake build of its own that fits, so sonde builds a small static library
from its encoder sources (and the parser, for the tests and later for actions), pinned to
a tagged release.

Firmware uses it with:

```cmake
add_subdirectory(sonde)
target_link_libraries(firmware PRIVATE sonde::sonde)
```

### Tests

- Encoding of each message type and each kind of `Data`, compared byte for byte with the
  CBOR of the `proto` crate (e.g. the motor and IMU fixtures in `proto/src/data.rs`).
- Shortest float widths, integer major types, `null`, `optional`.
- Batches: messages stay whole when a batch is full, the next batch opens, `Full` when none
  is free, `TooLarge`, `commit()`, `request_commit()`, `dropped()`.
- Wrong field and element counts give `Invalid` and leave the batch unchanged.
- Producer and consumer on two threads for a while (also under ThreadSanitizer): every
  batch the consumer takes decodes, and the messages taken plus `dropped()` add up to
  the messages traced.
- `FileSink` writes a recording the outpost's `read_frames` reads; a Rust test in `proto`
  decodes a recording written by sonde.
- CI builds the library also with `-fno-exceptions -fno-rtti` and for an Arm Cortex-M0
  and M4 target (`arm-none-eabi-gcc`), without the hosted parts.

## Changes needed in the outpost and blip

- **blip reads batches** (done): a WebSocket frame may hold several messages as a CBOR
  sequence, so a UDP datagram of sonde passes through the outpost as one frame.
- **Socket passthrough keeps messages whole.** Today the outpost forwards each raw TCP
  read (up to 4096 bytes) as one WebSocket frame, which may end in half a message. It
  also looks at the start of each frame only to keep the latest `Actions` and to drop
  `ActionResult`s, which misses them inside a batch. The socket protocol has to cut the
  TCP stream at message boundaries and look at each message of a batch.
- **Timestamp domain.** `proto` gets a way to say whether timestamps are uptime or UTC,
  see [Timestamps](#timestamps). Not needed for the first version.
- **Recording header.** Nothing changes: a sonde recording is a version 1 recording.

## Later: actions

Actions complete the `proto` mapping, as a second step:

```cpp
sonde::Action actions[] = {
    {"motor/speed", "Speed setpoint", {sonde::Param::float_("rpm", 0, 3000)}, "motor/speed"},
    {"reboot", "Restart the target", {}, nullptr, /*confirm=*/true},
};
tracer.announce(actions);  // proto: Message::Actions

// a Request from the outpost, decoded with tinycbor's parser
sonde::Invocation invocation;
if (sonde::decode_request(rx, rx_size, invocation)) {
    auto result = dispatch(invocation);   // the firmware's handler
    tracer.action_result(invocation.id, result);  // proto: Message::ActionResult
}
```

## Decisions

1. **No global tracer, no macros.** A global tracer would be shared by several contexts;
   each context gets its own tracer instance instead.
2. **Timestamps are uptime or UTC**, as the clock gives them; `proto` is updated later to
   tell them apart.
3. **Field names on the wire stay** (about 25 bytes per `Value`); a compact encoding would
   be a protocol change of all three components.
4. **Contexts are managed by the user** with tracer instances; a tracer is a lock-free
   single-producer single-consumer queue of batches.
5. **Recordings are uncompressed.**
