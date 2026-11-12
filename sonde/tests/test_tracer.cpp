// Tracers: what a tracer sends is what the queue sends with the full path of the tracer.

#include "cbor.hpp"

#include <sonde/file_sink.hpp>

#include <catch2/catch_test_macros.hpp>

#include <string>

using sonde::Level;
using sonde::Status;
using sonde::Timestamp;
using test::Bytes;

namespace {

/// The bytes of the messages traced by `trace` on a queue, with timestamps of 0.
template <typename F>
Bytes traced(F&& trace) {
    sonde::MemorySink sink;
    sonde::Storage<1024, 1> storage;
    sonde::Queue queue{storage, sink, nullptr};
    trace(queue);
    REQUIRE(queue.commit() == Status::Ok);
    return sink.bytes();
}

// static tracers are constant-initialized, as their queue
sonde::Storage<64, 1> constant_storage;
[[maybe_unused]] sonde::Queue constant_queue{constant_storage, nullptr};
[[maybe_unused]] constexpr sonde::Tracer constant_root{constant_queue};
[[maybe_unused]] constexpr sonde::Tracer constant_motor{constant_queue, "motor"};
[[maybe_unused]] constexpr sonde::Tracer constant_left{constant_motor, "left"};
static_assert(constant_left.parent() == &constant_motor);

// a child only keeps the address of its parent, so it is constant-initialized also when
// the parent is not declared constexpr, e.g. for an ISR that fires before the static
// constructors
sonde::Tracer dynamic_parent{constant_queue, "net"};
[[maybe_unused]] constexpr sonde::Tracer constant_child{dynamic_parent, "rx"};
[[maybe_unused]] constexpr sonde::Tracer constant_grandchild{constant_child, "crc"};
static_assert(constant_child.parent() == &dynamic_parent);
static_assert(constant_grandchild.parent() == &constant_child);

}  // namespace

TEST_CASE("a tracer sends values and units below its name") {
    CHECK(traced([](sonde::Queue& queue) {
              sonde::Tracer motor{queue, "motor"};
              CHECK(motor.unit("speed", "rpm") == Status::Ok);
              CHECK(motor.value("speed", 1500u) == Status::Ok);
              CHECK(motor.value(Timestamp{7}, "speed", 1501u) == Status::Ok);
              CHECK(motor.value("", 1.5) == Status::Ok);
          }) == traced([](sonde::Queue& queue) {
              queue.unit("motor/speed", "rpm");
              queue.value("motor/speed", 1500u);
              queue.value(Timestamp{7}, "motor/speed", 1501u);
              queue.value("motor", 1.5);
          }));
}

TEST_CASE("a tracer without a name sends the paths as they are") {
    CHECK(traced([](sonde::Queue& queue) {
              sonde::Tracer root{queue};
              root.value("cpu_load", 12.5f);
              root.info("booted");
          }) == traced([](sonde::Queue& queue) {
              queue.value("cpu_load", 12.5f);
              queue.log(Level::Info, "booted");
          }));
}

TEST_CASE("a tracer logs at its name, without a component") {
    CHECK(traced([](sonde::Queue& queue) {
              sonde::Tracer motor{queue, "motor"};
              motor.log(Level::Trace, "step");
              motor.log(Timestamp{3}, Level::Warn, "hot");
              motor.debug("a");
              motor.info("b");
              motor.warn("c");
              motor.error("d");
              motor.logf(Level::Info, "%d mA", 12);
              motor.debugf("%d", 1);
              motor.infof("%d", 2);
              motor.warnf("%d", 3);
              motor.errorf("%d", 4);
          }) == traced([](sonde::Queue& queue) {
              queue.log("motor", Level::Trace, "step");
              queue.log(Timestamp{3}, "motor", Level::Warn, "hot");
              queue.log("motor", Level::Debug, "a");
              queue.log("motor", Level::Info, "b");
              queue.log("motor", Level::Warn, "c");
              queue.log("motor", Level::Error, "d");
              queue.log("motor", Level::Info, "12 mA");
              queue.log("motor", Level::Debug, "1");
              queue.log("motor", Level::Info, "2");
              queue.log("motor", Level::Warn, "3");
              queue.log("motor", Level::Error, "4");
          }));
}

TEST_CASE("a child is below its parent") {
    CHECK(traced([](sonde::Queue& queue) {
              sonde::Tracer robot{queue, "robot"};
              sonde::Tracer arm{robot, "arm"};
              sonde::Tracer grip{arm, "grip"};
              sonde::Tracer root{queue};
              sonde::Tracer net{root, "net"};
              arm.value("angle", 0.5);
              arm.info("homed");
              grip.unit("force", "N");
              grip.warn("slipping");
              net.value("rx", 3u);
          }) == traced([](sonde::Queue& queue) {
              queue.value("robot/arm/angle", 0.5);
              queue.log("robot/arm", Level::Info, "homed");
              queue.unit("robot/arm/grip/force", "N");
              queue.log("robot/arm/grip", Level::Warn, "slipping");
              queue.value("net/rx", 3u);
          }));
}

TEST_CASE("a static child sends with the queue of its parent") {
    CHECK(&constant_grandchild.queue() == &constant_queue);
    CHECK(constant_child.value("bytes", 1u) == Status::Ok);
    CHECK(constant_queue.dropped() == 0);
}

TEST_CASE("a path longer than SONDE_PATH_MAX is dropped as too large") {
    sonde::MemorySink sink;
    sonde::Storage<1024, 1> storage;
    sonde::Queue queue{storage, sink, nullptr};
    const std::string long_name(60, 'a');
    sonde::Tracer parent{queue, long_name};
    sonde::Tracer child{parent, "b"};
    // 60 + 1 + 1 + 1 + 34 = 97 bytes, one more than the default of 96
    const std::string path(34, 'c');
    CHECK(child.value(path, 1u) == Status::TooLarge);
    CHECK(child.unit(path, "m") == Status::TooLarge);
    // 96 bytes fit
    CHECK(child.value(path.substr(1), 1u) == Status::Ok);
    CHECK(queue.dropped() == 2);
    CHECK(queue.commit() == Status::Ok);
    const auto messages = test::items(sink.bytes());
    REQUIRE(messages);
    CHECK(messages->size() == 1);
}

TEST_CASE("a log of a tracer is at its name and has no component key") {
    // {"Log": {"timestamp": 3, "path": "log/imu", "level": 3, "message": "hot"}}
    const Bytes expected = test::hex(
        "a1634c6f67"
        "a4" "6974696d657374616d70" "03"
        "6470617468" "676c6f672f696d75"
        "656c6576656c" "03"
        "676d657373616765" "63686f74");
    CHECK(traced([](sonde::Queue& queue) {
              sonde::Tracer imu{queue, "log/imu"};
              imu.log(Timestamp{3}, Level::Warn, "hot");
          }) == expected);
    CHECK(traced([](sonde::Queue& queue) {
              sonde::Tracer log{queue, "log"};
              sonde::Tracer imu{log, "imu"};
              imu.log(Timestamp{3}, Level::Warn, "hot");
          }) == expected);
}
