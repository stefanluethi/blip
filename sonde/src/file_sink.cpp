#include <sonde/file_sink.hpp>

#include <filesystem>
#include <system_error>

namespace sonde {

namespace {

// {"format": "blip recording", "version": 1}
constexpr std::uint8_t recording_header[] = {
    0xa2,                                                     // map of 2 pairs
    0x66, 'f', 'o', 'r', 'm', 'a', 't',                       // "format"
    0x6e, 'b', 'l', 'i', 'p', ' ', 'r', 'e', 'c', 'o', 'r', 'd', 'i', 'n', 'g',
    0x67, 'v', 'e', 'r', 's', 'i', 'o', 'n',                  // "version"
    0x01,
};

}  // namespace

FileSink::FileSink(const char* path) {
    const std::filesystem::path file{path};
    if (file.has_parent_path()) {
        std::error_code error;
        std::filesystem::create_directories(file.parent_path(), error);
    }
    file_ = std::fopen(path, "wb");
    if (file_ != nullptr && !write(recording_header, sizeof recording_header)) {
        std::fclose(file_);
        file_ = nullptr;
    }
}

FileSink::~FileSink() {
    if (file_ != nullptr) {
        std::fclose(file_);
    }
}

bool FileSink::is_open() const {
    return file_ != nullptr;
}

bool FileSink::write(const std::uint8_t* data, std::size_t size) {
    // flushed, so that the file holds the batches written if the program ends abruptly
    return file_ != nullptr && std::fwrite(data, 1, size, file_) == size &&
           std::fflush(file_) == 0;
}

bool MemorySink::write(const std::uint8_t* data, std::size_t size) {
    bytes_.insert(bytes_.end(), data, data + size);
    return true;
}

const std::vector<std::uint8_t>& MemorySink::bytes() const {
    return bytes_;
}

void MemorySink::clear() {
    bytes_.clear();
}

}  // namespace sonde
