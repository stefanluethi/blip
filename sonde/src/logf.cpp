// The printf-style logs of Queue and Tracer, in a file of their own: a program that
// does not call them does not link them, nor the vsnprintf of its C library.

#include <sonde/sonde.hpp>

#include <cstdarg>
#include <cstdio>

#ifndef SONDE_LOGF_MAX
#define SONDE_LOGF_MAX 128
#endif

static_assert(SONDE_LOGF_MAX >= 1, "SONDE_LOGF_MAX needs a byte for the NUL");

namespace sonde {

namespace {

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

/// Formats the message into `buffer`, cut to it at a whole character.
std::string_view formatted(char (&buffer)[SONDE_LOGF_MAX], const char* format,
                           std::va_list args) {
    const int length = std::vsnprintf(buffer, sizeof buffer, format, args);
    std::size_t size = 0;
    if (length > 0) {
        size = static_cast<std::size_t>(length);
        if (size >= sizeof buffer) {
            size = whole_utf8(buffer, sizeof buffer - 1);
        }
    }
    return std::string_view{buffer, size};
}

}  // namespace

Status Queue::logf(Level level, const char* format, ...) {
    std::va_list args;
    va_start(args, format);
    const Status status = logv({}, level, format, args);
    va_end(args);
    return status;
}

Status Queue::logf(std::string_view path, Level level, const char* format, ...) {
    std::va_list args;
    va_start(args, format);
    const Status status = logv(path, level, format, args);
    va_end(args);
    return status;
}

Status Queue::logv(std::string_view path, Level level, const char* format,
                   std::va_list args) {
    char buffer[SONDE_LOGF_MAX];
    return log_with(nullptr, path, level, formatted(buffer, format, args));
}

Status Tracer::logv(Level level, const char* format, std::va_list args) const {
    char buffer[SONDE_LOGF_MAX];
    return log(level, formatted(buffer, format, args));
}

Status Tracer::logf(Level level, const char* format, ...) const {
    std::va_list args;
    va_start(args, format);
    const Status status = logv(level, format, args);
    va_end(args);
    return status;
}

#define SONDE_LEVEL_LOGF(name, level)                           \
    Status Tracer::name(const char* format, ...) const {        \
        std::va_list args;                                      \
        va_start(args, format);                                 \
        const Status status = logv(Level::level, format, args); \
        va_end(args);                                           \
        return status;                                          \
    }

SONDE_LEVEL_LOGF(debugf, Debug)
SONDE_LEVEL_LOGF(infof, Info)
SONDE_LEVEL_LOGF(warnf, Warn)
SONDE_LEVEL_LOGF(errorf, Error)

#undef SONDE_LEVEL_LOGF

}  // namespace sonde
