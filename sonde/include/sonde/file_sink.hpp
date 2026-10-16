// Sinks of hosted builds: to a recording file and to memory.

#pragma once

#include <sonde/sonde.hpp>

#if !SONDE_HOSTED
#error "sonde/file_sink.hpp needs a hosted build (SONDE_HOSTED)"
#endif

#include <cstdio>
#include <vector>

namespace sonde {

/// Writes a blip recording: the header {"format": "blip recording", "version": 1} and
/// then the batches, uncompressed. blip opens the file directly, the outpost replays it.
/// Missing directories of the path are created.
class FileSink final : public Sink {
public:
    explicit FileSink(const char* path);
    ~FileSink();
    FileSink(const FileSink&) = delete;
    FileSink& operator=(const FileSink&) = delete;

    bool is_open() const;
    bool write(const std::uint8_t* data, std::size_t size) override;

private:
    std::FILE* file_ = nullptr;
};

/// Keeps the batches in memory, e.g. to check them in a unit test.
class MemorySink final : public Sink {
public:
    bool write(const std::uint8_t* data, std::size_t size) override;
    const std::vector<std::uint8_t>& bytes() const;
    void clear();

private:
    std::vector<std::uint8_t> bytes_;
};

}  // namespace sonde
