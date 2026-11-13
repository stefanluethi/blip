// Recording files of the FileSink.
//
// tests/data/recording.blip is a recording written by sonde, which the proto crate and
// the outpost read in their tests. Set SONDE_UPDATE_FIXTURES=1 to write it anew.

#include "cbor.hpp"

#include <sonde/file_sink.hpp>

#include <catch2/catch_test_macros.hpp>

#include <algorithm>
#include <array>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <optional>
#include <string>

using test::Bytes;
using test::hex;

namespace {

Bytes read(const std::filesystem::path& path) {
    std::ifstream file{path, std::ios::binary};
    return Bytes{std::istreambuf_iterator<char>{file}, std::istreambuf_iterator<char>{}};
}

constexpr sonde::Hello fixture_hello{"bench", sonde::TimeBase::Boot};

/// {"Hello": {"version": 1, "name": "bench", "clock": "boot", "sender": "sonde 0.2.0"}}
Bytes bench_hello() {
    return hex("a16548656c6c6fa46776657273696f6e01646e616d656562656e636865636c6f636b6462"
               "6f6f746673656e6465726b736f6e646520302e322e30");
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
void trace(sonde::Queue& queue) {
    steps = 0;
    queue.unit("motor/speed", "rpm");
    queue.value(sonde::Timestamp{1'000'000}, "motor/speed", 1500u);
    steps = 2;
    queue.value("cpu_load", 12.5f);
    queue.value("sensor/temperature", sonde::null);
    const float acc[3] = {0.5f, -1.0f, 9.81f};
    queue.value("imu", sonde::record(2, [&](sonde::Record& record) {
                     record.field("acc", acc);
                     record.field("valid", true);
                 }));
    queue.commit();
    steps = 3;
    queue.value("motor", Motor{1500, true, "running"});
    queue.value("offset", -42);
    queue.value("ratio", 0.1);
    queue.log("robot/arm", sonde::Level::Info, "started");
    queue.log(sonde::Level::Warn, "no path");
    queue.logf(sonde::Level::Error, "overrun %d", 3);
    queue.value("adc/samples", std::array<std::uint16_t, 4>{0, 1023, 2048, 4095});
    queue.commit();
    const sonde::Label states[] = {{-1, "fault"}, {0, "off"}, {1, "on"}};
    queue.metadata("motor/state", sonde::Metadata{{}, "Drive state", {states, 3}, std::nullopt});
    queue.metadata("motor/current", sonde::Metadata{"A", {}, {}, sonde::Range{-2.5, 10.0}});
    const std::uint16_t raw[3] = {512, 513, 511};
    queue.samples("adc/raw", raw, sonde::Timestamp{4'000'000}, sonde::Period{250'000});
    const sonde::Timestamp times[2] = {{4'000'000}, {4'500'000}};
    queue.samples("adc/peak", std::array<std::optional<float>, 2>{1.5f, std::nullopt}, times);
    queue.commit();
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
        sonde::Queue to_file{file_storage, file, &test_clock, fixture_hello};
        trace(to_file);

        sonde::Storage<128, 1> memory_storage;
        sonde::Queue to_memory{memory_storage, memory, &test_clock, fixture_hello};
        trace(to_memory);
    }

    const Bytes header = hex("a2" "66666f726d6174" "6e626c6970207265636f7264696e67"
                             "6776657273696f6e" "03");
    Bytes expected = header;
    expected.insert(expected.end(), memory.bytes().begin(), memory.bytes().end());
    CHECK(read(path) == expected);

    const auto items = test::items(memory.bytes());
    REQUIRE(items);
    CHECK(items->size() == 17);
    CHECK((*items)[0] == bench_hello());
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
        sonde::Queue queue{storage, file, &test_clock, fixture_hello};
        trace(queue);
    }
    CHECK(read(path) == read(fixture));
    const Bytes value_name = hex("6556616c7565");  // the text "Value"
    const Bytes bytes = read(fixture);
    CHECK(std::search(bytes.begin(), bytes.end(), value_name.begin(), value_name.end()) ==
          bytes.end());
}

TEST_CASE("a file sink that cannot open its file fails") {
    std::filesystem::create_directories("traces/directory.blip");
    sonde::FileSink file{"traces/directory.blip"};
    CHECK(!file.is_open());
    const std::uint8_t byte = 0xf6;
    CHECK(!file.write(&byte, 1));

    sonde::Storage<128, 1> storage;
    sonde::Queue queue{storage, file, nullptr};
    CHECK(queue.value("a", 1u) == sonde::Status::Ok);
    CHECK(queue.commit() == sonde::Status::SinkError);
    CHECK(queue.dropped() == 2);  // the Hello and the value
}

TEST_CASE("a memory sink clears") {
    sonde::MemorySink memory;
    const std::uint8_t byte = 0xf6;
    CHECK(memory.write(&byte, 1));
    CHECK(memory.bytes() == Bytes{0xf6});
    memory.clear();
    CHECK(memory.bytes().empty());
}
