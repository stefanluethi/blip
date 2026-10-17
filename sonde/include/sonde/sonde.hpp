// sonde: encodes traces as blip messages, in batches of CBOR sequences.
//
// See sonde/README.md for the concepts and the usage.

#pragma once

#include <array>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string_view>
#include <type_traits>
#include <utility>

#include <cbor.h>

#ifndef SONDE_HOSTED
#define SONDE_HOSTED 0
#endif

#if defined(__GNUC__)
#define SONDE_PRINTF(format_index, first_arg) \
    __attribute__((format(printf, format_index, first_arg)))
#else
#define SONDE_PRINTF(format_index, first_arg)
#endif

namespace sonde {

// --- Types --------------------------------------------------------------------------

/// Nanoseconds in the time domain of the clock: the uptime of a target or UTC (since
/// the Unix epoch).
struct Timestamp {
    std::uint64_t ns;
};

/// Returns the current time in nanoseconds. Called from the producer's context, so it
/// has to be safe there, e.g. in an ISR.
using Clock = std::uint64_t (*)();

#if SONDE_HOSTED
/// UTC in nanoseconds since the Unix epoch, from the system clock (hosted builds).
std::uint64_t system_clock_ns();
#endif

/// The default clock: UTC from the system clock in hosted builds, none (nullptr) on
/// targets, which pass their own. Without a clock, messages are timestamped 0.
constexpr Clock default_clock() {
#if SONDE_HOSTED
    return &system_clock_ns;
#else
    return nullptr;
#endif
}

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

// --- Encoding of values -------------------------------------------------------------

class Record;
class Array;
class Encoder;

namespace detail {
/// Encodes the object at the pointer, of a type the function knows.
using EncodeFn = void (*)(Encoder&, const void*);
}  // namespace detail

/// Encodes one Data item. It refers to the item being encoded, so it is not copied; the
/// same holds for Record and Array.
class Encoder {
public:
    Encoder(const Encoder&) = delete;
    Encoder& operator=(const Encoder&) = delete;

    template <typename T>
    void encode(const T& value);
    void null();
    /// A record of `fields` fields, filled by fill(Record&).
    template <typename F>
    void record(std::size_t fields, F&& fill);
    /// An array of `elements` elements, filled by fill(Array&).
    template <typename F>
    void array(std::size_t elements, F&& fill);

private:
    friend class Tracer;
    friend class Record;
    friend class Array;

    Encoder(CborEncoder* cbor, CborError* error) : cbor_{cbor}, error_{error} {}

    void boolean(bool value);
    void uint(std::uint64_t value);
    void sint(std::int64_t value);
    void floating(float value);
    void floating(double value);
    void string(const char* data, std::size_t size);
    /// Whether the container could be opened; it is closed by close().
    bool open_map(CborEncoder& map, std::size_t pairs);
    bool open_array(CborEncoder& array, std::size_t elements);
    void close(const CborEncoder& container);
    void check(CborError error);

    template <typename It>
    void elements(It begin, std::size_t size);

    CborEncoder* cbor_;
    /// The first error other than running out of space, shared by nested encoders.
    CborError* error_;
};

class Record {
public:
    Record(const Record&) = delete;
    Record& operator=(const Record&) = delete;

    template <typename T>
    void field(std::string_view name, const T& value) {
        encoder_.encode(name);
        encoder_.encode(value);
    }

private:
    friend class Encoder;
    Record(CborEncoder* cbor, CborError* error) : encoder_{cbor, error} {}
    Encoder encoder_;
};

class Array {
public:
    Array(const Array&) = delete;
    Array& operator=(const Array&) = delete;

    template <typename T>
    void add(const T& value) {
        encoder_.encode(value);
    }

private:
    friend class Encoder;
    Array(CborEncoder* cbor, CborError* error) : encoder_{cbor, error} {}
    Encoder encoder_;
};

/// A record built by a fill function, see record().
template <typename F>
struct RecordOf {
    std::size_t fields;
    F fill;

    friend void encode(Encoder& encoder, const RecordOf& record) {
        encoder.record(record.fields, record.fill);
    }
};

/// An array built by a fill function, see array().
template <typename F>
struct ArrayOf {
    std::size_t elements;
    F fill;

    friend void encode(Encoder& encoder, const ArrayOf& array) {
        encoder.array(array.elements, array.fill);
    }
};

/// An encodable record of `fields` fields, filled by fill(Record&), e.g. for value().
template <typename F>
RecordOf<F> record(std::size_t fields, F&& fill) {
    return {fields, std::forward<F>(fill)};
}

/// An encodable array of `elements` elements, filled by fill(Array&), e.g. for value().
template <typename F>
ArrayOf<F> array(std::size_t elements, F&& fill) {
    return {elements, std::forward<F>(fill)};
}

namespace detail {

template <typename T>
struct is_std_array : std::false_type {};
template <typename T, std::size_t N>
struct is_std_array<std::array<T, N>> : std::true_type {};

template <typename T>
struct is_optional : std::false_type {};
template <typename T>
struct is_optional<std::optional<T>> : std::true_type {};

template <typename T>
struct is_span : std::false_type {};
template <typename T>
struct is_span<Span<T>> : std::true_type {};

template <typename T>
inline constexpr bool is_char = std::is_same_v<T, char> || std::is_same_v<T, wchar_t> ||
#if defined(__cpp_char8_t)
                                std::is_same_v<T, char8_t> ||
#endif
                                std::is_same_v<T, char16_t> || std::is_same_v<T, char32_t>;

// Hides any sonde::encode, so that only argument-dependent lookup finds the encode
// functions of user types. Not deleted, since MSVC then skips the lookup in has_encode.
struct Hidden {};
void encode(Hidden);

template <typename T, typename = void>
struct has_encode : std::false_type {};
template <typename T>
struct has_encode<T, std::void_t<decltype(encode(std::declval<Encoder&>(),
                                                 std::declval<const T&>()))>>
    : std::true_type {};

template <typename T>
void encode_user_type(Encoder& encoder, const T& value) {
    encode(encoder, value);
}

}  // namespace detail

template <typename T>
void Encoder::encode(const T& value) {
    if constexpr (std::is_same_v<T, std::nullptr_t>) {
        null();
    } else if constexpr (std::is_same_v<T, bool>) {
        boolean(value);
    } else if constexpr (std::is_integral_v<T>) {
        static_assert(!detail::is_char<T>,
                      "a character is not encodable, cast it to an integer or pass a string");
        if constexpr (std::is_signed_v<T>) {
            sint(value);
        } else {
            uint(value);
        }
    } else if constexpr (std::is_floating_point_v<T>) {
        static_assert(!std::is_same_v<T, long double>, "long double is not encodable");
        floating(value);
    } else if constexpr (std::is_array_v<T>) {
        using Element = std::remove_cv_t<std::remove_extent_t<T>>;
        constexpr std::size_t size = std::extent_v<T>;
        if constexpr (std::is_same_v<Element, char>) {
            // a string literal or a buffer, up to the first NUL
            std::size_t length = 0;
            while (length < size && value[length] != '\0') {
                ++length;
            }
            string(value, length);
        } else {
            elements(value, size);
        }
    } else if constexpr (std::is_same_v<T, const char*> || std::is_same_v<T, char*>) {
        if (value == nullptr) {
            null();
        } else {
            encode(std::string_view{value});
        }
    } else if constexpr (std::is_convertible_v<const T&, std::string_view>) {
        const std::string_view view = value;
        string(view.data(), view.size());
    } else if constexpr (detail::is_std_array<T>::value) {
        elements(value.data(), value.size());
    } else if constexpr (detail::is_span<T>::value) {
        elements(value.data, value.size);
    } else if constexpr (detail::is_optional<T>::value) {
        if (value) {
            encode(*value);
        } else {
            null();
        }
    } else {
        static_assert(detail::has_encode<T>::value,
                      "the type is not encodable, declare "
                      "void encode(sonde::Encoder&, const T&) next to it");
        detail::encode_user_type(*this, value);
    }
}

template <typename F>
void Encoder::record(std::size_t fields, F&& fill) {
    CborEncoder container;
    if (!open_map(container, fields)) {
        return;
    }
    Record filled{&container, error_};
    fill(filled);
    close(container);
}

template <typename F>
void Encoder::array(std::size_t elements, F&& fill) {
    CborEncoder container;
    if (!open_array(container, elements)) {
        return;
    }
    Array filled{&container, error_};
    fill(filled);
    close(container);
}

template <typename It>
void Encoder::elements(It begin, std::size_t size) {
    CborEncoder container;
    if (!open_array(container, size)) {
        return;
    }
    Encoder element{&container, error_};
    for (std::size_t i = 0; i < size; ++i) {
        element.encode(begin[i]);
    }
    close(container);
}

// --- Storage and sinks --------------------------------------------------------------

/// The batches of one tracer: Batches slots of BatchBytes bytes, plus their sizes.
/// Static or on the stack, owned by the user, outlives the tracer.
template <std::size_t BatchBytes, std::size_t Batches = 2>
class Storage {
    static_assert(BatchBytes > 0, "a batch needs bytes");
    static_assert(Batches >= 1, "a tracer needs a batch");
    // the indices of the queue count up to 2 * Batches
    static_assert(Batches <= (std::uint32_t{1} << 30), "too many batches");

    friend class Tracer;
    std::uint8_t bytes_[Batches * BatchBytes];
    std::size_t sizes_[Batches];
};

/// Takes the batches of a tracer whose producer and consumer are one context.
class Sink {
public:
    /// Takes a batch of complete messages, a CBOR sequence. Returns false on failure.
    virtual bool write(const std::uint8_t* data, std::size_t size) = 0;

protected:
    ~Sink() = default;
};

// --- Tracer -------------------------------------------------------------------------

/// Encodes messages into batches. One tracer per context that traces.
///
/// The batches are a single-producer single-consumer queue: the producer traces, the
/// consumer takes the ready batches and releases them once they are sent. With a sink,
/// producer and consumer are one context and each ready batch goes to the sink.
///
/// The constructors are constexpr, so a static tracer is initialized before any code
/// runs, e.g. an ISR that fires before the static constructors.
class Tracer {
public:
    /// Producer and consumer: the consumer takes the batches.
    template <std::size_t BatchBytes, std::size_t Batches>
    constexpr explicit Tracer(Storage<BatchBytes, Batches>& storage,
                              Clock clock = default_clock())
        : Tracer{storage.bytes_, BatchBytes, Batches, storage.sizes_, nullptr, clock} {}

    /// One context: each ready batch goes to the sink right away.
    template <std::size_t BatchBytes, std::size_t Batches>
    constexpr Tracer(Storage<BatchBytes, Batches>& storage, Sink& sink,
                     Clock clock = default_clock())
        : Tracer{storage.bytes_, BatchBytes, Batches, storage.sizes_, &sink, clock} {}

    Tracer(const Tracer&) = delete;
    Tracer& operator=(const Tracer&) = delete;

    // --- Producer: only from the context that traces --------------------------------

    // proto: Message::Value, timestamped by the clock or explicitly
    template <typename T>
    Status value(std::string_view path, const T& value) {
        return value_with(nullptr, path, &encode_value<T>, &value);
    }
    template <typename T>
    Status value(Timestamp timestamp, std::string_view path, const T& value) {
        return value_with(&timestamp, path, &encode_value<T>, &value);
    }

    // proto: Message::ValueMetadata
    Status metadata(std::string_view path, std::string_view unit);

    // proto: Message::Log; an empty component is left out on the wire
    Status log(Level level, std::string_view component, std::string_view message);
    Status log(Timestamp timestamp, Level level, std::string_view component,
               std::string_view message);
    /// printf-like, formatted into a buffer of SONDE_LOGF_MAX bytes on the stack,
    /// longer messages are cut off.
    Status logf(Level level, std::string_view component, const char* format, ...)
        SONDE_PRINTF(4, 5);

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

private:
    template <typename T>
    static void encode_value(Encoder& encoder, const void* value) {
        encoder.encode(*static_cast<const T*>(value));
    }

    constexpr Tracer(std::uint8_t* bytes, std::size_t batch_bytes, std::size_t batches,
                     std::size_t* sizes, Sink* sink, Clock clock)
        : bytes_{bytes},
          batch_bytes_{batch_bytes},
          batches_{static_cast<std::uint32_t>(batches)},
          sizes_{sizes},
          sink_{sink},
          clock_{clock} {}

    // A timestamp of nullptr is read from the clock, once the message has a batch.
    Status value_with(const Timestamp* timestamp, std::string_view path,
                      detail::EncodeFn encode, const void* value);
    Status log_with(const Timestamp* timestamp, Level level, std::string_view component,
                    std::string_view message);
    /// Encodes a message into the open batch and commits on request. If `clock_time` is
    /// not nullptr, it is set from the clock before the message is encoded.
    Status emit(detail::EncodeFn encode, const void* message, Timestamp* clock_time);
    Status append(detail::EncodeFn encode, const void* message, Timestamp* clock_time);
    struct Encoded;
    Encoded encode_into(std::uint8_t* at, std::size_t space, detail::EncodeFn encode,
                        const void* message) const;
    /// Commits the open batch, without looking at the requests.
    Status commit_open();
    /// The open batch, or nullptr if all batches are ready or taken.
    std::uint8_t* open_batch() const;
    /// The batch of an index of the queue.
    std::uint32_t slot(std::uint32_t index) const {
        return index < batches_ ? index : index - batches_;
    }
    std::uint8_t* batch(std::uint32_t index) const {
        return bytes_ + slot(index) * batch_bytes_;
    }
    std::uint32_t next(std::uint32_t index) const {
        return index + 1 == 2 * batches_ ? 0 : index + 1;
    }
    void drop(std::uint32_t messages);

    std::uint8_t* const bytes_;
    const std::size_t batch_bytes_;
    const std::uint32_t batches_;
    std::size_t* const sizes_;
    Sink* const sink_;
    const Clock clock_;

    // Producer only.
    std::size_t open_size_ = 0;
    std::uint32_t open_messages_ = 0;
    std::uint32_t requests_seen_ = 0;

    // Each index counts modulo 2 * batches_ and is written by one side only: the batches
    // [read_, write_) are ready or taken, the batch at write_ is open unless all are.
    std::atomic<std::uint32_t> write_{0};  // producer
    std::atomic<std::uint32_t> read_{0};   // consumer
    std::atomic<std::uint32_t> requests_{0};  // consumer: commit requests made
    std::atomic<std::uint32_t> dropped_{0};   // producer

    // The queue only loads and stores the atomics, it needs no read-modify-write. GCC
    // emits plain loads and stores with barriers for atomics of up to a word, also where
    // read-modify-write needs a library call and is_always_lock_free is false, e.g. on
    // ARMv6-M (Cortex-M0/M0+) or RISC-V without the A extension. An int of 32 bits
    // stands for a word of 32 bits; on 8- and 16-bit targets the loads are not atomic.
#if defined(__GNUC__) && !defined(__clang__)
    static constexpr bool word_loads_and_stores = sizeof(int) >= sizeof(std::uint32_t);
#else
    static constexpr bool word_loads_and_stores = false;
#endif
    static_assert(std::atomic<std::uint32_t>::is_always_lock_free || word_loads_and_stores,
                  "sonde needs lock-free loads and stores of 32-bit atomics");
};

}  // namespace sonde
