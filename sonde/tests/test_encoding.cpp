// The encoding of the messages and of each kind of Data, compared byte for byte with the
// CBOR of the proto crate (serde_cbor) and of the outpost.

#include "cbor.hpp"

#include <sonde/file_sink.hpp>

#include <catch2/catch_test_macros.hpp>

#include <algorithm>
#include <array>
#include <cmath>
#include <cstring>
#include <limits>
#include <optional>
#include <string>
#include <type_traits>

using sonde::Status;
using test::Bytes;
using test::hex;

namespace {

/// The bytes of the messages traced by `trace`, with timestamps of 0.
template <typename F>
Bytes traced(F&& trace) {
    sonde::MemorySink sink;
    sonde::Storage<1024, 1> storage;
    sonde::Queue queue{storage, sink, nullptr};
    trace(queue);
    REQUIRE(queue.commit() == Status::Ok);
    const auto bytes = test::without_hello(sink.bytes());
    REQUIRE(bytes.has_value());
    return *bytes;
}

/// The bytes of `value` as the Data of a Sample message.
template <typename T>
Bytes encoded(const T& value) {
    const Bytes message = traced([&](sonde::Queue& queue) {
        REQUIRE(queue.value("a", value) == Status::Ok);
    });
    const Bytes prefix = test::value_prefix();
    REQUIRE(message.size() > prefix.size());
    REQUIRE(Bytes(message.begin(), message.begin() + static_cast<std::ptrdiff_t>(prefix.size())) ==
            prefix);
    return Bytes(message.begin() + static_cast<std::ptrdiff_t>(prefix.size()), message.end());
}

}  // namespace

// The encoders refer to the item being encoded, a copy would outlive it.
static_assert(!std::is_copy_constructible_v<sonde::Encoder>);
static_assert(!std::is_copy_assignable_v<sonde::Encoder>);
static_assert(!std::is_copy_constructible_v<sonde::Record>);
static_assert(!std::is_copy_constructible_v<sonde::Array>);
static_assert(!std::is_move_constructible_v<sonde::Record>);

// REQ_PROTO_LEVELS: the levels are the numbers proto documents.
static_assert(static_cast<int>(sonde::Level::Trace) == 0);
static_assert(static_cast<int>(sonde::Level::Debug) == 1);
static_assert(static_cast<int>(sonde::Level::Info) == 2);
static_assert(static_cast<int>(sonde::Level::Warn) == 3);
static_assert(static_cast<int>(sonde::Level::Error) == 4);

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

TEST_CASE("samples are the messages of proto") {
    // the fixtures of proto/src/data.rs, of the outpost
    const auto motor = traced([](sonde::Queue& queue) {
        queue.value(sonde::Timestamp{1}, "motor", motors::Motor{1500, true, "running"});
    });
    CHECK(motor == hex("a16653616d706c65a36974696d657374616d70016470617468656d6f746f726576616c7565a3"
                       "6973706565645f72706d1905dc67656e61626c6564f56573746174656772756e6e696e67"));

    const auto temperature = traced([](sonde::Queue& queue) {
        queue.value(sonde::Timestamp{3}, "sensor/temperature", sonde::null);
    });
    CHECK(temperature ==
          hex("a16653616d706c65a36974696d657374616d700364706174687273656e736f722f74656d"
              "70657261747572656576616c7565f6"));

    // the IMU fixture with floats in their shortest width, as serde_cbor writes them
    const auto imu = traced([](sonde::Queue& queue) {
        const double acc[3] = {0.5, -1.0, 9.81};
        queue.value(sonde::Timestamp{2}, "imu", sonde::record(1, [&](sonde::Record& record) {
                         record.field("acc", acc);
                     }));
    });
    CHECK(imu == hex("a16653616d706c65a36974696d657374616d7002647061746863696d756576616c7565a16361"
                     "636383f93800f9bc00fb40239eb851eb851f"));
}

TEST_CASE("timestamps come from the clock") {
    sonde::MemorySink sink;
    sonde::Storage<256, 1> storage;
    sonde::Queue queue{storage, sink, [] { return std::uint64_t{1}; }};
    queue.value("motor", motors::Motor{1500, true, "running"});
    queue.commit();
    CHECK(test::without_hello(sink.bytes()) ==
          hex("a16653616d706c65a36974696d657374616d70016470617468656d6f746f726576616c7565a3"
              "6973706565645f72706d1905dc67656e61626c6564f56573746174656772756e6e696e67"));
}

TEST_CASE("a unit is trace metadata") {
    CHECK(traced([](sonde::Queue& queue) { queue.unit("speed", "rpm"); }) ==
          hex("a16d54726163654d65746164617461"  // {"TraceMetadata":
              "a2" "6470617468" "657370656564"  // {"path": "speed",
              "64756e6974" "6372706d"));        // "unit": "rpm"}}
    // an empty unit is left out
    CHECK(traced([](sonde::Queue& queue) { queue.unit("speed", ""); }) ==
          hex("a16d54726163654d65746164617461a16470617468657370656564"));
}

TEST_CASE("trace metadata leaves out what is not declared") {
    using sonde::Label;
    using sonde::Metadata;
    using sonde::Range;
    const Label stop[] = {{0, "stop"}};
    CHECK(traced([&](sonde::Queue& queue) {
              queue.metadata("speed", Metadata{"rpm", "Motor speed", {stop, 1}, Range{0.0, 3000.0}});
          }) ==
          hex("a16d54726163654d65746164617461a5647061746865737065656464756e69746372706d6b"
              "6465736372697074696f6e6b4d6f746f7220737065656464656e756da1006473746f706572"
              "616e6765a2636d696ef90000636d6178f969dc"));
    const Label states[] = {{-1, "fault"}, {0, "off"}, {1, "on"}};
    CHECK(traced([&](sonde::Queue& queue) {
              queue.metadata("motor/state", Metadata{{}, "Drive state", {states, 3}, std::nullopt});
          }) ==
          hex("a16d54726163654d65746164617461a364706174686b6d6f746f722f73746174656b64657363"
              "72697074696f6e6b447269766520737461746564656e756da320656661756c7400636f666601"
              "626f6e"));
    CHECK(traced([](sonde::Queue& queue) {
              queue.metadata("motor/current", Metadata{"A", {}, {}, Range{-2.5, 10.0}});
          }) ==
          hex("a16d54726163654d65746164617461a364706174686d6d6f746f722f63757272656e746475"
              "6e697461416572616e6765a2636d696ef9c100636d6178f94900"));
    const Label wide[] = {{-1000, "m"}, {1000, "k"}};
    CHECK(traced([&](sonde::Queue& queue) {
              queue.metadata("a", Metadata{{}, {}, {wide, 2}, Range{0.1, 1e300}});
          }) ==
          hex("a16d54726163654d65746164617461a36470617468616164656e756da23903e7616d1903e8"
              "616b6572616e6765a2636d696efb3fb999999999999a636d6178fb7e37e43c8800759c"));
    // an empty Metadata is an empty unit, a unit is a Metadata with only a unit
    CHECK(traced([](sonde::Queue& queue) { queue.metadata("speed", Metadata{}); }) ==
          traced([](sonde::Queue& queue) { queue.unit("speed", ""); }));
    CHECK(traced([](sonde::Queue& queue) { queue.unit("speed", "rpm"); }) ==
          traced([](sonde::Queue& queue) { queue.metadata("speed", Metadata{"rpm"}); }));
}

TEST_CASE("sonde writes the new names and no old one") {
    const Bytes bytes = traced([](sonde::Queue& queue) {
        queue.value("a", 1u);
        queue.unit("a", "m");
        queue.log(sonde::Level::Info, "x");
    });
    const Bytes value = hex("6556616c7565");
    const Bytes value_metadata = hex("6d56616c75654d65746164617461");
    CHECK(std::search(bytes.begin(), bytes.end(), value.begin(), value.end()) == bytes.end());
    CHECK(std::search(bytes.begin(), bytes.end(), value_metadata.begin(),
                      value_metadata.end()) == bytes.end());
    const auto parts = test::items(bytes);
    REQUIRE(parts.has_value());
    REQUIRE(parts->size() == 3);
    const Bytes sample = hex("a16653616d706c65");
    const Bytes trace_metadata = hex("a16d54726163654d65746164617461");
    CHECK(std::equal(sample.begin(), sample.end(), (*parts)[0].begin()));
    CHECK(std::equal(trace_metadata.begin(), trace_metadata.end(), (*parts)[1].begin()));
}

TEST_CASE("logs have no component and leave out an empty path") {
    // {"Log": {"timestamp": 3, "path": "log/imu", "level": 3, "message": "hot"}}
    CHECK(traced([](sonde::Queue& queue) {
              queue.log(sonde::Timestamp{3}, "log/imu", sonde::Level::Warn, "hot");
          }) == hex("a1634c6f67"
                    "a4" "6974696d657374616d70" "03"
                    "6470617468" "676c6f672f696d75"
                    "656c6576656c" "03"
                    "676d657373616765" "63686f74"));
    // the same at a path given without a timestamp, read from the clock (0 here)
    CHECK(traced([](sonde::Queue& queue) {
              queue.log("log/imu", sonde::Level::Warn, "hot");
          }) == hex("a1634c6f67"
                    "a4" "6974696d657374616d70" "00"
                    "6470617468" "676c6f672f696d75"
                    "656c6576656c" "03"
                    "676d657373616765" "63686f74"));
    CHECK(traced([](sonde::Queue& queue) {
              queue.log(sonde::Level::Error, "hot");
          }) == hex("a1634c6f67"
                    "a3" "6974696d657374616d70" "00"
                    "656c6576656c" "04"
                    "676d657373616765" "63686f74"));
    CHECK(traced([](sonde::Queue& queue) {
              queue.log(sonde::Timestamp{3}, sonde::Level::Warn, "hot");
          }) == hex("a1634c6f67"
                    "a3" "6974696d657374616d70" "03"
                    "656c6576656c" "03"
                    "676d657373616765" "63686f74"));
}

TEST_CASE("logf formats into a message") {
    CHECK(traced([](sonde::Queue& queue) {
              queue.logf(sonde::Level::Info, "%d mA", 12);
          }) == traced([](sonde::Queue& queue) {
              queue.log(sonde::Level::Info, "12 mA");
          }));
    CHECK(traced([](sonde::Queue& queue) {
              queue.logf("arm", sonde::Level::Info, "%d mA", 12);
          }) == traced([](sonde::Queue& queue) {
              queue.log("arm", sonde::Level::Info, "12 mA");
          }));
}

TEST_CASE("logf cuts off long messages before a UTF-8 sequence") {
    // 126 bytes and a 2-byte sequence, of which the buffer of 128 bytes keeps 127
    const std::string text = std::string(126, 'x') + "\xc3\xa4" "x";
    CHECK(traced([&](sonde::Queue& queue) {
              queue.logf(sonde::Level::Info, "%s", text.c_str());
          }) == traced([](sonde::Queue& queue) {
              queue.log(sonde::Level::Info, std::string(126, 'x'));
          }));
    const std::string ascii(200, 'y');
    CHECK(traced([&](sonde::Queue& queue) {
              queue.logf(sonde::Level::Info, "%s", ascii.c_str());
          }) == traced([](sonde::Queue& queue) {
              queue.log(sonde::Level::Info, std::string(127, 'y'));
          }));
}

// --- Invalid values -----------------------------------------------------------------

TEST_CASE("wrong counts are invalid and leave the batch unchanged") {
    sonde::MemorySink sink;
    sonde::Storage<256, 1> storage;
    sonde::Queue queue{storage, sink, nullptr};
    REQUIRE(queue.value("a", 1u) == Status::Ok);

    CHECK(queue.value("a", sonde::record(2, [](sonde::Record& record) {
              record.field("x", 1);
          })) == Status::Invalid);
    CHECK(queue.value("a", sonde::record(1, [](sonde::Record& record) {
              record.field("x", 1);
              record.field("y", 2);
          })) == Status::Invalid);
    CHECK(queue.value("a", sonde::array(3, [](sonde::Array& array) {
              array.add(1);
          })) == Status::Invalid);
    CHECK(queue.value("a", sonde::array(1, [](sonde::Array& array) {
              array.add(sonde::array(2, [](sonde::Array& inner) { inner.add(2); }));
          })) == Status::Invalid);

    CHECK(queue.commit() == Status::Ok);
    Bytes expected = test::value_prefix();
    expected.push_back(0x01);
    CHECK(test::without_hello(sink.bytes()) == expected);
    CHECK(queue.dropped() == 0);
}

// --- Blocks of samples --------------------------------------------------------------

static_assert(sonde::detail::is_sequence<int[2]>);
static_assert(!sonde::detail::is_sequence<char[4]>);
static_assert(!sonde::detail::is_sequence<int>);
static_assert(sonde::detail::is_sequence<sonde::Span<float>>);
static_assert(sonde::detail::is_timestamps<const sonde::Timestamp[2]>::value);
static_assert(!sonde::detail::is_timestamps<std::uint64_t[2]>::value);

TEST_CASE("samples at a period") {
    const std::uint16_t raw[3] = {512, 513, 511};
    const Bytes expected = hex(
        "a16753616d706c6573a46470617468636164636676616c756573831902001902011901ff"
        "6974696d657374616d701a003d090066706572696f641a0003d090");
    CHECK(expected.size() == 63);
    const sonde::Timestamp start{4'000'000};
    const sonde::Period period{250'000};
    CHECK(traced([&](sonde::Queue& queue) {
              CHECK(queue.samples("adc", raw, start, period) == Status::Ok);
          }) == expected);
    CHECK(traced([&](sonde::Queue& queue) {
              const std::array<std::uint16_t, 3> values{512, 513, 511};
              CHECK(queue.samples("adc", values, start, period) == Status::Ok);
          }) == expected);
    CHECK(traced([&](sonde::Queue& queue) {
              const sonde::Span<std::uint16_t> values{raw, 3};
              CHECK(queue.samples("adc", values, start, period) == Status::Ok);
          }) == expected);

    CHECK(traced([](sonde::Queue& queue) {
              CHECK(queue.samples("a",
                                  sonde::array(2,
                                               [](sonde::Array& array) {
                                                   array.add(0.5);
                                                   array.add(-1.0);
                                               }),
                                  sonde::Timestamp{0}, sonde::Period{1000}) == Status::Ok);
          }) == hex("a16753616d706c6573a4647061746861616676616c75657382f93800f9bc00"
                    "6974696d657374616d700066706572696f641903e8"));

    const Bytes empty = traced([](sonde::Queue& queue) {
        CHECK(queue.samples("a", std::array<int, 0>{}, sonde::Timestamp{1},
                            sonde::Period{2}) == Status::Ok);
    });
    CHECK(empty == hex("a16753616d706c6573a4647061746861616676616c756573806974696d657374616d70"
                       "0166706572696f6402"));
    CHECK(empty.size() == 44);
}

TEST_CASE("samples at timestamps") {
    const sonde::Timestamp times[2] = {{4'000'000}, {4'500'000}};
    const Bytes expected = hex(
        "a16753616d706c6573a36470617468647065616b6676616c75657382f93e00f6"
        "6a74696d657374616d7073821a003d09001a0044aa20");
    CHECK(expected.size() == 54);
    const std::array<std::optional<float>, 2> values{1.5f, std::nullopt};
    CHECK(traced([&](sonde::Queue& queue) {
              CHECK(queue.samples("peak", values, times) == Status::Ok);
          }) == expected);
    CHECK(traced([&](sonde::Queue& queue) {
              const std::array<sonde::Timestamp, 2> stamps{{{4'000'000}, {4'500'000}}};
              CHECK(queue.samples("peak", values, stamps) == Status::Ok);
          }) == expected);
    CHECK(traced([&](sonde::Queue& queue) {
              const sonde::Span<sonde::Timestamp> stamps{times, 2};
              CHECK(queue.samples("peak", values, stamps) == Status::Ok);
          }) == expected);

    const Bytes empty = traced([](sonde::Queue& queue) {
        CHECK(queue.samples("a", std::array<int, 0>{},
                            sonde::Span<sonde::Timestamp>{nullptr, 0}) == Status::Ok);
    });
    CHECK(empty == hex("a16753616d706c6573a3647061746861616676616c756573806a74696d657374616d707380"));
    CHECK(empty.size() == 37);
}

TEST_CASE("samples need as many timestamps as values") {
    sonde::MemorySink sink;
    sonde::Storage<256, 1> storage;
    sonde::Queue queue{storage, sink, nullptr};
    const sonde::Timestamp times[2] = {{1}, {2}};
    CHECK(queue.samples("a", std::array<int, 3>{1, 2, 3}, times) == Status::Invalid);
    CHECK(queue.samples("a", sonde::array(1, [](sonde::Array& array) { array.add(1); }),
                        times) == Status::Invalid);
    CHECK(queue.commit() == Status::Ok);
    CHECK(sink.bytes().empty());
    CHECK(queue.dropped() == 0);
}
