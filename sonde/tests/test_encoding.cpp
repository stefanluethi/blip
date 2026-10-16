// The encoding of the messages and of each kind of Data, compared byte for byte with the
// CBOR of the proto crate (serde_cbor) and of the outpost.

#include "cbor.hpp"

#include <sonde/file_sink.hpp>

#include <catch2/catch_test_macros.hpp>

#include <array>
#include <cmath>
#include <cstring>
#include <limits>
#include <optional>
#include <string>

using sonde::Status;
using test::Bytes;
using test::hex;

namespace {

/// The bytes of the messages traced by `trace`, with timestamps of 0.
template <typename F>
Bytes traced(F&& trace) {
    sonde::MemorySink sink;
    sonde::Storage<1024, 1> storage;
    sonde::Tracer tracer{storage, sink, nullptr};
    trace(tracer);
    REQUIRE(tracer.commit() == Status::Ok);
    return sink.bytes();
}

/// The bytes of `value` as the Data of a Value message.
template <typename T>
Bytes encoded(const T& value) {
    const Bytes message = traced([&](sonde::Tracer& tracer) {
        REQUIRE(tracer.value("a", value) == Status::Ok);
    });
    const Bytes prefix = test::value_prefix();
    REQUIRE(message.size() > prefix.size());
    REQUIRE(Bytes(message.begin(), message.begin() + static_cast<std::ptrdiff_t>(prefix.size())) ==
            prefix);
    return Bytes(message.begin() + static_cast<std::ptrdiff_t>(prefix.size()), message.end());
}

}  // namespace

// --- Data ---------------------------------------------------------------------------

TEST_CASE("null") {
    CHECK(encoded(nullptr) == hex("f6"));
    CHECK(encoded(sonde::null) == hex("f6"));
    CHECK(encoded(std::optional<int>{}) == hex("f6"));
    CHECK(encoded(std::optional<int>{5}) == hex("05"));
    CHECK(encoded(static_cast<const char*>(nullptr)) == hex("f6"));
}

TEST_CASE("integers take the major type of their sign") {
    CHECK(encoded(5u) == hex("05"));
    CHECK(encoded(5) == hex("05"));
    CHECK(encoded(-5) == hex("24"));
    CHECK(encoded(std::uint8_t{200}) == hex("18c8"));
    CHECK(encoded(std::int8_t{-128}) == hex("387f"));
    CHECK(encoded(std::uint16_t{1500}) == hex("1905dc"));
    CHECK(encoded(std::numeric_limits<std::uint64_t>::max()) == hex("1bffffffffffffffff"));
    CHECK(encoded(std::numeric_limits<std::int64_t>::min()) == hex("3b7fffffffffffffff"));
}

TEST_CASE("bools") {
    CHECK(encoded(true) == hex("f5"));
    CHECK(encoded(false) == hex("f4"));
}

TEST_CASE("floats take the shortest width that keeps the value") {
    CHECK(encoded(1.5) == hex("f93e00"));
    CHECK(encoded(1.5f) == hex("f93e00"));
    CHECK(encoded(12.5f) == hex("f94a40"));
    CHECK(encoded(0.0) == hex("f90000"));
    CHECK(encoded(-0.0) == hex("f98000"));
    CHECK(encoded(65504.0f) == hex("f97bff"));          // the largest half
    CHECK(encoded(65520.0f) == hex("fa477ff000"));      // rounds to infinity as a half
    CHECK(encoded(std::ldexp(1.0f, -24)) == hex("f90001"));  // the smallest half
    CHECK(encoded(std::ldexp(3.0f, -24)) == hex("f90003"));
    CHECK(encoded(std::ldexp(1.0f, -25)) == hex("fa33000000"));
    CHECK(encoded(std::ldexp(1.0f, -14)) == hex("f90400"));  // the smallest normal half
    CHECK(encoded(0.1f) == hex("fa3dcccccd"));
    CHECK(encoded(0.1) == hex("fb3fb999999999999a"));
    CHECK(encoded(-0.1) == hex("fbbfb999999999999a"));
    CHECK(encoded(1e300) == hex("fb7e37e43c8800759c"));
    CHECK(encoded(std::numeric_limits<double>::infinity()) == hex("f97c00"));
    CHECK(encoded(-std::numeric_limits<float>::infinity()) == hex("f9fc00"));
    CHECK(encoded(std::numeric_limits<double>::quiet_NaN()) == hex("f97e00"));
}

TEST_CASE("strings") {
    CHECK(encoded("ab") == hex("626162"));
    CHECK(encoded(std::string_view{"ab"}) == hex("626162"));
    CHECK(encoded(std::string{"ab"}) == hex("626162"));
    const char* pointer = "ab";
    CHECK(encoded(pointer) == hex("626162"));
    char buffer[8] = "ab";  // up to the first NUL
    CHECK(encoded(buffer) == hex("626162"));
    CHECK(encoded("") == hex("60"));
}

TEST_CASE("arrays") {
    const int plain[3] = {1, -1, 2};
    CHECK(encoded(plain) == hex("830120" "02"));
    CHECK(encoded(std::array<bool, 2>{true, false}) == hex("82f5f4"));
    const float floats[2] = {0.5f, -1.0f};
    CHECK(encoded(sonde::Span<float>{floats, 2}) == hex("82f93800f9bc00"));
    const int nested[2][2] = {{1, 2}, {3, 4}};
    CHECK(encoded(nested) == hex("82820102820304"));
    CHECK(encoded(sonde::array(3, [](sonde::Array& array) {
              for (int i = 0; i < 3; ++i) {
                  array.add(i * 0.5);
              }
          })) == hex("83f90000f93800f93c00"));
    CHECK(encoded(std::array<int, 0>{}) == hex("80"));
}

TEST_CASE("records keep the order of their fields") {
    // proto: composites_are_plain_arrays_and_maps
    CHECK(encoded(sonde::record(2, [](sonde::Record& record) {
              record.field("x", 1u);
              record.field("y", sonde::array(2, [](sonde::Array& array) {
                               array.add(false);
                               array.add(sonde::null);
                           }));
          })) == hex("a2617801617982f4f6"));
}

namespace motors {

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

struct Position {
    double x;
    double y;
};

void encode(sonde::Encoder& encoder, const Position& position) {
    const double coordinates[2] = {position.x, position.y};
    encoder.encode(coordinates);
}

}  // namespace motors

TEST_CASE("user types encode with an encode function next to them") {
    CHECK(encoded(motors::Position{0.5, 1.0}) == hex("82f93800f93c00"));
    const motors::Position path[2] = {{0.0, 0.0}, {1.0, 0.5}};
    CHECK(encoded(path) == hex("8282f90000f90000" "82f93c00f93800"));
    CHECK(encoded(std::optional<motors::Position>{}) == hex("f6"));
}

// --- Messages -----------------------------------------------------------------------

TEST_CASE("values are the messages of proto") {
    // the fixtures of proto/src/data.rs, of the outpost
    const auto motor = traced([](sonde::Tracer& tracer) {
        tracer.value(sonde::Timestamp{1}, "motor", motors::Motor{1500, true, "running"});
    });
    CHECK(motor == hex("a16556616c7565a36974696d657374616d70016470617468656d6f746f726576616c7565a3"
                       "6973706565645f72706d1905dc67656e61626c6564f56573746174656772756e6e696e67"));

    const auto temperature = traced([](sonde::Tracer& tracer) {
        tracer.value(sonde::Timestamp{3}, "sensor/temperature", sonde::null);
    });
    CHECK(temperature ==
          hex("a16556616c7565a36974696d657374616d700364706174687273656e736f722f74656d"
              "70657261747572656576616c7565f6"));

    // the IMU fixture with floats in their shortest width, as serde_cbor writes them
    const auto imu = traced([](sonde::Tracer& tracer) {
        const double acc[3] = {0.5, -1.0, 9.81};
        tracer.value(sonde::Timestamp{2}, "imu", sonde::record(1, [&](sonde::Record& record) {
                         record.field("acc", acc);
                     }));
    });
    CHECK(imu == hex("a16556616c7565a36974696d657374616d7002647061746863696d756576616c7565a16361"
                     "636383f93800f9bc00fb40239eb851eb851f"));
}

TEST_CASE("timestamps come from the clock") {
    sonde::MemorySink sink;
    sonde::Storage<256, 1> storage;
    sonde::Tracer tracer{storage, sink, [] { return std::uint64_t{1}; }};
    tracer.value("motor", motors::Motor{1500, true, "running"});
    tracer.commit();
    CHECK(sink.bytes() ==
          hex("a16556616c7565a36974696d657374616d70016470617468656d6f746f726576616c7565a3"
              "6973706565645f72706d1905dc67656e61626c6564f56573746174656772756e6e696e67"));
}

TEST_CASE("metadata") {
    CHECK(traced([](sonde::Tracer& tracer) { tracer.metadata("speed", "rpm"); }) ==
          hex("a16d56616c75654d65746164617461"  // {"ValueMetadata":
              "a2" "6470617468" "657370656564"  // {"path": "speed",
              "64756e6974" "6372706d"));            // "unit": "rpm"}}
}

TEST_CASE("logs leave out an empty component") {
    CHECK(traced([](sonde::Tracer& tracer) {
              tracer.log(sonde::Timestamp{3}, sonde::Level::Warn, "motor", "hot");
          }) == hex("a1634c6f67"                                // {"Log":
                    "a4" "6974696d657374616d70" "03"          // {"timestamp": 3,
                    "656c6576656c" "03"                        // "level": 3,
                    "69636f6d706f6e656e74" "656d6f746f72"      // "component": "motor",
                    "676d657373616765" "63686f74"));            // "message": "hot"}}
    CHECK(traced([](sonde::Tracer& tracer) {
              tracer.log(sonde::Level::Error, "", "hot");
          }) == hex("a1634c6f67"
                    "a3" "6974696d657374616d70" "00"
                    "656c6576656c" "04"
                    "676d657373616765" "63686f74"));
}

TEST_CASE("logf formats into a message") {
    CHECK(traced([](sonde::Tracer& tracer) {
              tracer.logf(sonde::Level::Info, "", "%d mA", 12);
          }) == traced([](sonde::Tracer& tracer) {
              tracer.log(sonde::Level::Info, "", "12 mA");
          }));
}

TEST_CASE("logf cuts off long messages before a UTF-8 sequence") {
    // 126 bytes and a 2-byte sequence, of which the buffer of 128 bytes keeps 127
    const std::string text = std::string(126, 'x') + "\xc3\xa4" "x";
    CHECK(traced([&](sonde::Tracer& tracer) {
              tracer.logf(sonde::Level::Info, "", "%s", text.c_str());
          }) == traced([](sonde::Tracer& tracer) {
              tracer.log(sonde::Level::Info, "", std::string(126, 'x'));
          }));
    const std::string ascii(200, 'y');
    CHECK(traced([&](sonde::Tracer& tracer) {
              tracer.logf(sonde::Level::Info, "", "%s", ascii.c_str());
          }) == traced([](sonde::Tracer& tracer) {
              tracer.log(sonde::Level::Info, "", std::string(127, 'y'));
          }));
}

// --- Invalid values -----------------------------------------------------------------

TEST_CASE("wrong counts are invalid and leave the batch unchanged") {
    sonde::MemorySink sink;
    sonde::Storage<256, 1> storage;
    sonde::Tracer tracer{storage, sink, nullptr};
    REQUIRE(tracer.value("a", 1u) == Status::Ok);

    CHECK(tracer.value("a", sonde::record(2, [](sonde::Record& record) {
              record.field("x", 1);
          })) == Status::Invalid);
    CHECK(tracer.value("a", sonde::record(1, [](sonde::Record& record) {
              record.field("x", 1);
              record.field("y", 2);
          })) == Status::Invalid);
    CHECK(tracer.value("a", sonde::array(3, [](sonde::Array& array) {
              array.add(1);
          })) == Status::Invalid);
    CHECK(tracer.value("a", sonde::array(1, [](sonde::Array& array) {
              array.add(sonde::array(2, [](sonde::Array& inner) { inner.add(2); }));
          })) == Status::Invalid);

    CHECK(tracer.commit() == Status::Ok);
    Bytes expected = test::value_prefix();
    expected.push_back(0x01);
    CHECK(sink.bytes() == expected);
    CHECK(tracer.dropped() == 0);
}
