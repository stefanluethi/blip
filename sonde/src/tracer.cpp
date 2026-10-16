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

namespace {

#if SONDE_HOSTED
std::uint64_t system_clock_ns() {
    using namespace std::chrono;
    return static_cast<std::uint64_t>(
        duration_cast<nanoseconds>(system_clock::now().time_since_epoch()).count());
}
#endif

/// A value encoded by a function, the data of a Value message.
struct Erased {
    void (*encode)(Encoder&, const void*);
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

Clock default_clock() {
#if SONDE_HOSTED
    return &system_clock_ns;
#else
    return nullptr;
#endif
}

Tracer::Tracer(std::uint8_t* bytes, std::size_t batch_bytes, std::size_t batches,
               std::size_t* sizes, Sink* sink, Clock clock)
    : bytes_{bytes},
      batch_bytes_{batch_bytes},
      batches_{static_cast<std::uint32_t>(batches)},
      sizes_{sizes},
      sink_{sink},
      clock_{clock} {}

// --- Producer -----------------------------------------------------------------------

Status Tracer::metadata(std::string_view path, std::string_view unit) {
    const MetadataMessage message{path, unit};
    return emit(&write_metadata, &message);
}

Status Tracer::log(Level level, std::string_view component, std::string_view message) {
    return log(now(), level, component, message);
}

Status Tracer::log(Timestamp timestamp, Level level, std::string_view component,
                   std::string_view message) {
    const LogMessage log{timestamp, level, component, message};
    return emit(&write_log, &log);
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

Timestamp Tracer::now() const {
    return Timestamp{clock_ != nullptr ? clock_() : 0};
}

Status Tracer::value_with(Timestamp timestamp, std::string_view path, EncodeValue encode,
                          const void* value) {
    const ValueMessage message{timestamp, path, Erased{encode, value}};
    return emit(&write_value, &message);
}

Status Tracer::emit(EncodeMessage encode, const void* message) {
    Status status = append(encode, message);
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

Status Tracer::append(EncodeMessage encode, const void* message) {
    std::uint8_t* batch = open_batch();
    if (batch == nullptr) {
        drop(1);
        return Status::Full;
    }

    // Each message gets a fresh encoder on the free part of the open batch, which only
    // grows by the message once it is encoded whole.
    std::uint8_t* const free = batch + open_size_;
    const std::size_t space = batch_bytes_ - open_size_;
    CborError error = CborNoError;
    CborEncoder cbor;
    cbor_encoder_init(&cbor, free, space, 0);
    Encoder encoder{&cbor, &error};
    encode(encoder, message);
    if (error != CborNoError) {
        return Status::Invalid;
    }
    const std::size_t missing = cbor_encoder_get_extra_bytes_needed(&cbor);
    if (missing == 0) {
        open_size_ += cbor_encoder_get_buffer_size(&cbor, free);
        ++open_messages_;
        return Status::Ok;
    }
    if (space + missing > batch_bytes_) {
        drop(1);
        return Status::TooLarge;
    }

    // It fits into an empty batch, so the open one holds messages: the next one gets it.
    const Status committed = commit_open();
    batch = open_batch();
    if (batch == nullptr) {
        drop(1);
        return Status::Full;
    }
    cbor_encoder_init(&cbor, batch, batch_bytes_, 0);
    encode(encoder, message);
    open_size_ = cbor_encoder_get_buffer_size(&cbor, batch);
    open_messages_ = 1;
    return committed;
}

Status Tracer::commit_open() {
    if (open_size_ == 0) {
        return Status::Ok;
    }
    const std::uint32_t write = write_.load(std::memory_order_relaxed);
    const std::uint32_t slot = write < batches_ ? write : write - batches_;
    Status status = Status::Ok;
    if (sink_ != nullptr) {
        if (!sink_->write(bytes_ + slot * batch_bytes_, open_size_)) {
            drop(open_messages_);
            status = Status::SinkError;
        }
    } else {
        sizes_[slot] = open_size_;
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
    const std::uint32_t slot = write < batches_ ? write : write - batches_;
    return bytes_ + slot * batch_bytes_;
}

std::uint32_t Tracer::next(std::uint32_t index) const {
    return index + 1 == 2 * batches_ ? 0 : index + 1;
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
    const std::uint32_t slot = read < batches_ ? read : read - batches_;
    return Batch{bytes_ + slot * batch_bytes_, sizes_[slot]};
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
