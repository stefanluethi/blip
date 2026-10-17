#include <sonde/sonde.hpp>

#include <cstdarg>
#include <cstdio>

#if SONDE_HOSTED
#include <chrono>
#endif

#ifndef SONDE_LOGF_MAX
#define SONDE_LOGF_MAX 128
#endif

static_assert(SONDE_LOGF_MAX >= 1, "SONDE_LOGF_MAX needs a byte for the NUL");

namespace sonde {

#if SONDE_HOSTED
std::uint64_t system_clock_ns() {
    using namespace std::chrono;
    return static_cast<std::uint64_t>(
        duration_cast<nanoseconds>(system_clock::now().time_since_epoch()).count());
}
#endif

namespace {

/// A value encoded by a function, the data of a Value message.
struct Erased {
    detail::EncodeFn encode;
    const void* value;
};

void encode(Encoder& encoder, const Erased& erased) {
    erased.encode(encoder, erased.value);
}

// The messages as serde encodes proto::Message: a map of the variant name to a map of
// the fields, in their order in proto.

struct ValueMessage {
    Timestamp timestamp;
    std::string_view path;
    Erased value;
};

void write_value(Encoder& encoder, const void* message) {
    const auto& value = *static_cast<const ValueMessage*>(message);
    encoder.record(1, [&](Record& variant) {
        variant.field("Value", record(3, [&](Record& fields) {
                          fields.field("timestamp", value.timestamp.ns);
                          fields.field("path", value.path);
                          fields.field("value", value.value);
                      }));
    });
}

struct MetadataMessage {
    std::string_view path;
    std::string_view unit;
};

void write_metadata(Encoder& encoder, const void* message) {
    const auto& metadata = *static_cast<const MetadataMessage*>(message);
    encoder.record(1, [&](Record& variant) {
        variant.field("ValueMetadata", record(2, [&](Record& fields) {
                          fields.field("path", metadata.path);
                          fields.field("unit", metadata.unit);
                      }));
    });
}

struct LogMessage {
    Timestamp timestamp;
    Level level;
    std::string_view component;
    std::string_view message;
};

void write_log(Encoder& encoder, const void* message) {
    const auto& log = *static_cast<const LogMessage*>(message);
    encoder.record(1, [&](Record& variant) {
        // the component has a default in proto, an empty one is left out
        const std::size_t count = log.component.empty() ? 3 : 4;
        variant.field("Log", record(count, [&](Record& fields) {
                          fields.field("timestamp", log.timestamp.ns);
                          fields.field("level", static_cast<std::uint8_t>(log.level));
                          if (!log.component.empty()) {
                              fields.field("component", log.component);
                          }
                          fields.field("message", log.message);
                      }));
    });
}

/// The length of the first `size` bytes of `text` without a UTF-8 sequence cut off at
/// their end.
std::size_t whole_utf8(const char* text, std::size_t size) {
    std::size_t start = size;
    while (start > 0 && (static_cast<unsigned char>(text[start - 1]) & 0xc0u) == 0x80u) {
        --start;
    }
    if (start == 0) {
        return size;
    }
    const auto lead = static_cast<unsigned char>(text[start - 1]);
    const std::size_t length = lead >= 0xf0u ? 4 : lead >= 0xe0u ? 3 : lead >= 0xc0u ? 2 : 1;
    return size - (start - 1) < length ? start - 1 : size;
}

}  // namespace

/// A message encoded into a batch, or what kept it out.
struct Tracer::Encoded {
    /// Ok if it is encoded, or Invalid; TooLarge if it does not fit into the space.
    Status status;
    /// The bytes it takes, also if it does not fit.
    std::size_t size;
};

// --- Producer -----------------------------------------------------------------------

Status Tracer::metadata(std::string_view path, std::string_view unit) {
    const MetadataMessage message{path, unit};
    return emit(&write_metadata, &message, nullptr);
}

Status Tracer::log(Level level, std::string_view component, std::string_view message) {
    return log_with(nullptr, level, component, message);
}

Status Tracer::log(Timestamp timestamp, Level level, std::string_view component,
                   std::string_view message) {
    return log_with(&timestamp, level, component, message);
}

Status Tracer::logf(Level level, std::string_view component, const char* format, ...) {
    char buffer[SONDE_LOGF_MAX];
    std::va_list args;
    va_start(args, format);
    const int length = std::vsnprintf(buffer, sizeof buffer, format, args);
    va_end(args);

    std::size_t size = 0;
    if (length > 0) {
        size = static_cast<std::size_t>(length);
        if (size >= sizeof buffer) {
            size = whole_utf8(buffer, sizeof buffer - 1);
        }
    }
    return log(level, component, std::string_view{buffer, size});
}

Status Tracer::commit() {
    requests_seen_ = requests_.load(std::memory_order_acquire);
    return commit_open();
}

Status Tracer::value_with(const Timestamp* timestamp, std::string_view path,
                          detail::EncodeFn encode, const void* value) {
    ValueMessage message{timestamp != nullptr ? *timestamp : Timestamp{0}, path,
                         Erased{encode, value}};
    return emit(&write_value, &message, timestamp == nullptr ? &message.timestamp : nullptr);
}

Status Tracer::log_with(const Timestamp* timestamp, Level level, std::string_view component,
                        std::string_view message) {
    LogMessage log{timestamp != nullptr ? *timestamp : Timestamp{0}, level, component,
                   message};
    return emit(&write_log, &log, timestamp == nullptr ? &log.timestamp : nullptr);
}

Status Tracer::emit(detail::EncodeFn encode, const void* message, Timestamp* clock_time) {
    Status status = append(encode, message, clock_time);
    const std::uint32_t requests = requests_.load(std::memory_order_acquire);
    if (requests != requests_seen_) {
        requests_seen_ = requests;
        const Status committed = commit_open();
        if (status == Status::Ok) {
            status = committed;
        }
    }
    return status;
}

Status Tracer::append(detail::EncodeFn encode, const void* message, Timestamp* clock_time) {
    std::uint8_t* open = open_batch();
    if (open == nullptr) {
        drop(1);
        return Status::Full;
    }
    // read only now, so that a full queue costs no reading of the clock
    if (clock_time != nullptr && clock_ != nullptr) {
        *clock_time = Timestamp{clock_()};
    }

    // Each message gets a fresh encoder on the free part of the open batch, which only
    // grows by the message once it is encoded whole.
    const std::size_t space = batch_bytes_ - open_size_;
    Encoded encoded = encode_into(open + open_size_, space, encode, message);
    if (encoded.status == Status::Ok) {
        open_size_ += encoded.size;
        ++open_messages_;
        return Status::Ok;
    }
    if (encoded.status == Status::Invalid) {
        return Status::Invalid;
    }
    if (encoded.size > batch_bytes_) {
        drop(1);
        return Status::TooLarge;
    }

    // It fits into an empty batch, so the open one holds messages: the next one gets it.
    const Status committed = commit_open();
    open = open_batch();
    if (open == nullptr) {
        drop(1);
        return Status::Full;
    }
    // Encoded anew, since a message is encoded at its place. The checks hold if the
    // encoding differs the second time, e.g. of a value read from hardware.
    encoded = encode_into(open, batch_bytes_, encode, message);
    if (encoded.status == Status::Ok) {
        open_size_ = encoded.size;
        open_messages_ = 1;
        return committed;
    }
    if (encoded.status == Status::TooLarge) {
        drop(1);
    }
    return encoded.status;
}

Tracer::Encoded Tracer::encode_into(std::uint8_t* at, std::size_t space,
                                    detail::EncodeFn encode, const void* message) const {
    CborError error = CborNoError;
    CborEncoder cbor;
    cbor_encoder_init(&cbor, at, space, 0);
    Encoder encoder{&cbor, &error};
    encode(encoder, message);
    if (error != CborNoError) {
        return {Status::Invalid, 0};
    }
    // tinycbor goes on counting the bytes needed when it runs out of space
    const std::size_t missing = cbor_encoder_get_extra_bytes_needed(&cbor);
    if (missing != 0) {
        return {Status::TooLarge, space + missing};
    }
    return {Status::Ok, cbor_encoder_get_buffer_size(&cbor, at)};
}

Status Tracer::commit_open() {
    if (open_size_ == 0) {
        return Status::Ok;
    }
    const std::uint32_t write = write_.load(std::memory_order_relaxed);
    Status status = Status::Ok;
    if (sink_ != nullptr) {
        if (!sink_->write(batch(write), open_size_)) {
            drop(open_messages_);
            status = Status::SinkError;
        }
    } else {
        sizes_[slot(write)] = open_size_;
        write_.store(next(write), std::memory_order_release);
    }
    open_size_ = 0;
    open_messages_ = 0;
    return status;
}

std::uint8_t* Tracer::open_batch() const {
    const std::uint32_t write = write_.load(std::memory_order_relaxed);
    if (sink_ == nullptr) {
        // acquire: the consumer is done with the batches it released
        const std::uint32_t read = read_.load(std::memory_order_acquire);
        const std::uint32_t ready = (write + 2 * batches_ - read) % (2 * batches_);
        if (ready >= batches_) {
            return nullptr;
        }
    }
    return batch(write);
}

void Tracer::drop(std::uint32_t messages) {
    dropped_.store(dropped_.load(std::memory_order_relaxed) + messages,
                   std::memory_order_relaxed);
}

// --- Consumer -----------------------------------------------------------------------

Batch Tracer::take() {
    const std::uint32_t read = read_.load(std::memory_order_relaxed);
    // acquire: the bytes and the size of a committed batch are written
    if (read == write_.load(std::memory_order_acquire)) {
        return Batch{nullptr, 0};
    }
    return Batch{batch(read), sizes_[slot(read)]};
}

void Tracer::release() {
    const std::uint32_t read = read_.load(std::memory_order_relaxed);
    if (read == write_.load(std::memory_order_acquire)) {
        return;
    }
    read_.store(next(read), std::memory_order_release);
}

void Tracer::request_commit() {
    requests_.store(requests_.load(std::memory_order_relaxed) + 1, std::memory_order_release);
}

// --- Either context -----------------------------------------------------------------

std::uint32_t Tracer::dropped() const {
    return dropped_.load(std::memory_order_relaxed);
}

}  // namespace sonde
