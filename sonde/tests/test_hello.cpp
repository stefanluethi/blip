// The Hello a queue sends before its first message.

#include "cbor.hpp"

#include <sonde/file_sink.hpp>

#include <catch2/catch_test_macros.hpp>

#include <string>

using sonde::Hello;
using sonde::Status;
using sonde::TimeBase;
using test::Bytes;
using test::hex;

namespace {

Bytes bench_hello() {
    return hex("a16548656c6c6fa46776657273696f6e01646e616d656562656e636865636c6f636b6462"
               "6f6f746673656e6465726b736f6e646520302e322e30");
}

Bytes concat(Bytes first, const Bytes& second) {
    first.insert(first.end(), second.begin(), second.end());
    return first;
}

/// The first item that a queue with `hello` sends before its value.
Bytes first_item(const Hello& hello) {
    sonde::MemorySink sink;
    sonde::Storage<256, 1> storage;
    sonde::Queue queue{storage, sink, nullptr, hello};
    queue.value("a", 1u);
    queue.commit();
    const auto items = test::items(sink.bytes());
    REQUIRE(items);
    REQUIRE(items->size() == 2);
    return (*items)[0];
}

}  // namespace

TEST_CASE("a queue with a Hello sends it before its first message") {
    sonde::MemorySink sink;
    sonde::Storage<256, 1> storage;
    sonde::Queue queue{storage, sink, nullptr, Hello{"bench", TimeBase::Boot}};
    queue.value("a", 1u);
    queue.value("a", 1u);
    queue.commit();
    Bytes expected = bench_hello();
    for (int i = 0; i < 2; ++i) {
        expected = concat(expected, test::value_prefix());
        expected.push_back(0x01);
    }
    CHECK(sink.bytes() == expected);
}

TEST_CASE("a queue without a Hello sends the default one") {
    Bytes expected = test::default_hello();
    expected = concat(expected, test::value_prefix());
    expected.push_back(0x01);
    {
        sonde::MemorySink sink;
        sonde::Storage<256, 1> storage;
        sonde::Queue queue{storage, sink, nullptr};
        queue.value("a", 1u);
        queue.commit();
        CHECK(sink.bytes() == expected);
    }
    {
        sonde::Storage<256, 1> storage;
        sonde::Queue queue{storage, nullptr};
        queue.value("a", 1u);
        queue.commit();
        const sonde::Batch batch = queue.take();
        REQUIRE(batch);
        CHECK(Bytes(batch.data, batch.data + batch.size) == expected);
    }
}

TEST_CASE("a Hello leaves out an empty name and an unknown clock") {
    CHECK(first_item(Hello{}) == test::default_hello());
    CHECK(first_item(Hello{"app", TimeBase::Epoch}) ==
          hex("a16548656c6c6fa46776657273696f6e01646e616d656361707065636c6f636b6565706f6368"
              "6673656e6465726b736f6e646520302e322e30"));
    CHECK(first_item(Hello{{}, TimeBase::Boot}) ==
          hex("a16548656c6c6fa36776657273696f6e0165636c6f636b64626f6f746673656e6465726b736f"
              "6e646520302e322e30"));
}

TEST_CASE("a Hello goes before a message of any kind") {
    sonde::MemorySink sink;
    sonde::Storage<256, 1> storage;
    sonde::Queue queue{storage, sink, nullptr, Hello{"bench", TimeBase::Boot}};
    queue.unit("a", "m");
    queue.commit();
    const auto items = test::items(sink.bytes());
    REQUIRE(items);
    REQUIRE(items->size() == 2);
    CHECK((*items)[0] == bench_hello());
    const Bytes metadata = hex("a16d54726163654d65746164617461");
    CHECK(std::equal(metadata.begin(), metadata.end(), (*items)[1].begin()));
}

TEST_CASE("a consumer takes the Hello first") {
    sonde::Storage<256, 2> storage;
    sonde::Queue queue{storage, nullptr, Hello{"bench", TimeBase::Boot}};
    queue.log(sonde::Level::Info, "x");
    queue.commit();
    const sonde::Batch batch = queue.take();
    REQUIRE(batch);
    const auto items = test::items(batch.data, batch.size);
    REQUIRE(items);
    REQUIRE(items->size() == 2);
    CHECK((*items)[0] == bench_hello());
}

TEST_CASE("a Hello larger than a batch is dropped once") {
    sonde::Storage<34, 2> storage;
    sonde::Queue queue{storage, nullptr, Hello{"bench", TimeBase::Boot}};
    CHECK(queue.value("a", 1u) == Status::Ok);
    CHECK(queue.dropped() == 1);
    CHECK(queue.value("a", 2u) == Status::Ok);  // commits the first batch
    CHECK(queue.dropped() == 1);
    queue.commit();
    for (std::uint64_t expected = 1; expected <= 2; ++expected) {
        const sonde::Batch batch = queue.take();
        REQUIRE(batch);
        const auto items = test::items(batch.data, batch.size);
        REQUIRE(items);
        REQUIRE(items->size() == 1);
        CHECK(!test::is_hello((*items)[0]));
        CHECK(test::uint_value((*items)[0]) == expected);
        queue.release();
    }
    CHECK(!queue.take());
}

TEST_CASE("the version of sonde is the one of its CMake project") {
    CHECK(std::string{SONDE_VERSION} == SONDE_CMAKE_VERSION);
}

// A static queue with a Hello is constant-initialized.
namespace {
sonde::Storage<64, 1> hello_storage;
[[maybe_unused]] constexpr sonde::Queue constant_with_hello{
    hello_storage, nullptr, sonde::Hello{"bench", sonde::TimeBase::Boot}};
}  // namespace
