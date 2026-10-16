// Recording files of the FileSink.
//
// tests/data/recording.blip is a recording written by sonde, which the proto crate and
// the outpost read in their tests. Set SONDE_UPDATE_FIXTURES=1 to write it anew.

#include "cbor.hpp"

#include <sonde/file_sink.hpp>

#include <catch2/catch_test_macros.hpp>

#include <array>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>

using test::Bytes;
using test::hex;

namespace {

Bytes read(const std::filesystem::path& path) {
    std::ifstream file{path, std::ios::binary};
    return Bytes{std::istreambuf_iterator<char>{file}, std::istreambuf_iterator<char>{}};
}

std::uint64_t steps = 0;
std::uint64_t test_clock() {
    return steps * 1'000'000;  // 1 ms per step, an "uptime"
}

struct Motor {
    std::uint32_t speed_rpm;
    bool enabled;
    const char* state;
};

void encode(sonde::Encoder& encoder, const Motor& motor) {
    encoder.record(3, [&](sonde::Record& record) {
        record.field("speed_rpm", motor.speed_rpm);
        record.field("enabled", motor.enabled);
        record.field("state", motor.state);
    });
}

/// The messages of the fixture, in batches of up to 128 bytes.
void trace(sonde::Tracer& tracer) {
    steps = 0;
    tracer.metadata("motor/speed", "rpm");
    tracer.value(sonde::Timestamp{1'000'000}, "motor/speed", 1500u);
    steps = 2;
    tracer.value("cpu_load", 12.5f);
    tracer.value("sensor/temperature", sonde::null);
    const float acc[3] = {0.5f, -1.0f, 9.81f};
    tracer.value("imu", sonde::record(2, [&](sonde::Record& record) {
                     record.field("acc", acc);
                     record.field("valid", true);
                 }));
    tracer.commit();
    steps = 3;
    tracer.value("motor", Motor{1500, true, "running"});
    tracer.value("offset", -42);
    tracer.value("ratio", 0.1);
    tracer.log(sonde::Level::Info, "motor", "started");
    tracer.log(sonde::Level::Warn, "", "no component");
    tracer.logf(sonde::Level::Error, "adc", "overrun %d", 3);
    tracer.value("adc/samples", std::array<std::uint16_t, 4>{0, 1023, 2048, 4095});
    tracer.commit();
}

}  // namespace

TEST_CASE("a file sink writes a recording") {
    const auto path = std::filesystem::path{"traces"} / "file_sink.blip";
    std::filesystem::remove(path);
    sonde::MemorySink memory;
    {
        sonde::FileSink file{path.string().c_str()};  // creates the directory
        REQUIRE(file.is_open());
        sonde::Storage<128, 1> file_storage;
        sonde::Tracer to_file{file_storage, file, &test_clock};
        trace(to_file);

        sonde::Storage<128, 1> memory_storage;
        sonde::Tracer to_memory{memory_storage, memory, &test_clock};
        trace(to_memory);
    }

    const Bytes header = hex("a2" "66666f726d6174" "6e626c6970207265636f7264696e67"
                             "6776657273696f6e" "01");
    Bytes expected = header;
    expected.insert(expected.end(), memory.bytes().begin(), memory.bytes().end());
    CHECK(read(path) == expected);

    const auto items = test::items(memory.bytes());
    REQUIRE(items);
    CHECK(items->size() == 12);
}

TEST_CASE("the recording fixture is what sonde writes") {
    const std::filesystem::path fixture = std::filesystem::path{SONDE_TEST_DATA} /
                                          "recording.blip";
    const char* update = std::getenv("SONDE_UPDATE_FIXTURES");
    const auto path = update != nullptr && std::string{update} == "1"
                          ? fixture
                          : std::filesystem::path{"traces"} / "recording.blip";
    {
        sonde::FileSink file{path.string().c_str()};
        REQUIRE(file.is_open());
        sonde::Storage<128, 1> storage;
        sonde::Tracer tracer{storage, file, &test_clock};
        trace(tracer);
    }
    CHECK(read(path) == read(fixture));
}

TEST_CASE("a file sink that cannot open its file fails") {
    std::filesystem::create_directories("traces/directory.blip");
    sonde::FileSink file{"traces/directory.blip"};
    CHECK(!file.is_open());
    const std::uint8_t byte = 0xf6;
    CHECK(!file.write(&byte, 1));

    sonde::Storage<64, 1> storage;
    sonde::Tracer tracer{storage, file, nullptr};
    tracer.value("a", 1u);
    CHECK(tracer.commit() == sonde::Status::SinkError);
    CHECK(tracer.dropped() == 1);
}

TEST_CASE("a memory sink clears") {
    sonde::MemorySink memory;
    const std::uint8_t byte = 0xf6;
    CHECK(memory.write(&byte, 1));
    CHECK(memory.bytes() == Bytes{0xf6});
    memory.clear();
    CHECK(memory.bytes().empty());
}
