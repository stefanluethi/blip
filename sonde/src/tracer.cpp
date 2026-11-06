#include <sonde/sonde.hpp>

#include <cstdarg>
#include <cstring>

#ifndef SONDE_PATH_MAX
#define SONDE_PATH_MAX 96
#endif

static_assert(SONDE_PATH_MAX >= 1, "SONDE_PATH_MAX needs a byte");

namespace sonde {

namespace {

/// Appends `part` to the path in `buffer`, after a "/" unless the path is empty; false if
/// it does not fit.
bool append(char* buffer, std::size_t size, std::size_t& length, std::string_view part) {
    if (part.empty()) {
        return true;
    }
    const std::size_t separator = length > 0 ? 1 : 0;
    if (part.size() + separator > size - length) {
        return false;
    }
    if (separator != 0) {
        buffer[length++] = '/';
    }
    std::memcpy(buffer + length, part.data(), part.size());
    length += part.size();
    return true;
}

}  // namespace

bool Tracer::write_name(char* buffer, std::size_t size, std::size_t& length) const {
    if (parent_ != nullptr && !parent_->write_name(buffer, size, length)) {
        return false;
    }
    return append(buffer, size, length, name_);
}

bool Tracer::full_path(std::string_view path, char* buffer, std::size_t size,
                       std::string_view& full) const {
    // a tracer without a parent needs no joining unless both have a name
    if (parent_ == nullptr && (name_.empty() || path.empty())) {
        full = name_.empty() ? path : name_;
        return full.size() <= size;
    }
    std::size_t length = 0;
    if (!write_name(buffer, size, length) || !append(buffer, size, length, path)) {
        return false;
    }
    full = std::string_view{buffer, length};
    return true;
}

Status Tracer::too_long() {
    queue_->drop(1);
    return Status::TooLarge;
}

Status Tracer::value_with(const Timestamp* timestamp, std::string_view path,
                          detail::EncodeFn encode, const void* value) {
    char buffer[SONDE_PATH_MAX];
    std::string_view full;
    if (!full_path(path, buffer, sizeof buffer, full)) {
        return too_long();
    }
    return queue_->value_with(timestamp, full, encode, value);
}

Status Tracer::unit(std::string_view path, std::string_view unit) {
    char buffer[SONDE_PATH_MAX];
    std::string_view full;
    if (!full_path(path, buffer, sizeof buffer, full)) {
        return too_long();
    }
    return queue_->unit(full, unit);
}

Status Tracer::log(Level level, std::string_view message) {
    char buffer[SONDE_PATH_MAX];
    std::string_view path;
    if (!full_path({}, buffer, sizeof buffer, path)) {
        return too_long();
    }
    return queue_->log_with(nullptr, path, level, component_, message);
}

Status Tracer::log(Timestamp timestamp, Level level, std::string_view message) {
    char buffer[SONDE_PATH_MAX];
    std::string_view path;
    if (!full_path({}, buffer, sizeof buffer, path)) {
        return too_long();
    }
    return queue_->log_with(&timestamp, path, level, component_, message);
}

Status Tracer::logv(Level level, const char* format, std::va_list args) {
    char buffer[SONDE_PATH_MAX];
    std::string_view path;
    if (!full_path({}, buffer, sizeof buffer, path)) {
        return too_long();
    }
    return queue_->logv(path, level, component_, format, args);
}

Status Tracer::logf(Level level, const char* format, ...) {
    std::va_list args;
    va_start(args, format);
    const Status status = logv(level, format, args);
    va_end(args);
    return status;
}

#define SONDE_LEVEL_LOGF(name, level)                         \
    Status Tracer::name(const char* format, ...) {            \
        std::va_list args;                                    \
        va_start(args, format);                               \
        const Status status = logv(Level::level, format, args); \
        va_end(args);                                         \
        return status;                                        \
    }

SONDE_LEVEL_LOGF(debugf, Debug)
SONDE_LEVEL_LOGF(infof, Info)
SONDE_LEVEL_LOGF(warnf, Warn)
SONDE_LEVEL_LOGF(errorf, Error)

#undef SONDE_LEVEL_LOGF

}  // namespace sonde
