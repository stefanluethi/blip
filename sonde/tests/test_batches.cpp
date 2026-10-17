// The batches of a tracer: whole messages, the queue between producer and consumer and
// the sinks.

#include "cbor.hpp"

#include <sonde/file_sink.hpp>

#include <catch2/catch_test_macros.hpp>

#include <string>

using sonde::Status;
using test::Bytes;

namespace {

// value("a", n) for n < 24 takes 33 bytes without a clock
constexpr std::size_t message_bytes = 33;

/// The values of the messages of a batch.
std::vector<std::uint64_t> values(sonde::Batch batch) {
    REQUIRE(batch);
    const auto items = test::items(batch.data, batch.size);
    REQUIRE(items);
    std::vector<std::uint64_t> result;
    for (const auto& item : *items) {
        const auto value = test::uint_value(item);
        REQUIRE(value);
        result.push_back(*value);
    }
    return result;
}

using Values = std::vector<std::uint64_t>;

struct FailingSink final : sonde::Sink {
    bool write(const std::uint8_t*, std::size_t) override { return false; }
};

}  // namespace

TEST_CASE("a message takes the expected bytes") {
    sonde::MemorySink sink;
    sonde::Storage<64, 1> storage;
    sonde::Tracer tracer{storage, sink, nullptr};
    tracer.value("a", 1u);
    tracer.commit();
    CHECK(sink.bytes().size() == message_bytes);
}

TEST_CASE("batches are ready on commit") {
    sonde::Storage<256, 2> storage;
    sonde::Tracer tracer{storage, nullptr};

    CHECK(!tracer.take());
    CHECK(tracer.commit() == Status::Ok);  // nothing to commit
    CHECK(!tracer.take());

    tracer.value("a", 1u);
    tracer.value("a", 2u);
    CHECK(!tracer.take());
    CHECK(tracer.commit() == Status::Ok);

    const sonde::Batch batch = tracer.take();
    CHECK(batch.size == 2 * message_bytes);
    CHECK(values(batch) == Values{1, 2});
    CHECK(tracer.take().data == batch.data);  // the same until released
    tracer.release();
    CHECK(!tracer.take());
    tracer.release();  // nothing taken, nothing to release
    CHECK(!tracer.take());
}

TEST_CASE("a message that does not fit goes to the next batch") {
    sonde::Storage<2 * message_bytes + 10, 2> storage;
    sonde::Tracer tracer{storage, nullptr};

    CHECK(tracer.value("a", 1u) == Status::Ok);
    CHECK(tracer.value("a", 2u) == Status::Ok);
    CHECK(tracer.value("a", 3u) == Status::Ok);  // commits the first batch
    CHECK(tracer.value("a", 4u) == Status::Ok);
    CHECK(tracer.value("a", 5u) == Status::Full);  // commits the second, none is free
    CHECK(tracer.value("a", 6u) == Status::Full);
    CHECK(tracer.commit() == Status::Ok);
    CHECK(tracer.dropped() == 2);

    CHECK(values(tracer.take()) == Values{1, 2});
    tracer.release();
    CHECK(tracer.value("a", 7u) == Status::Ok);  // into the released batch
    CHECK(values(tracer.take()) == Values{3, 4});
    tracer.release();
    CHECK(!tracer.take());
    CHECK(tracer.commit() == Status::Ok);
    CHECK(values(tracer.take()) == Values{7});
    tracer.release();
    CHECK(tracer.dropped() == 2);
}

TEST_CASE("a message larger than a batch is dropped") {
    sonde::Storage<message_bytes + 4, 2> storage;
    sonde::Tracer tracer{storage, nullptr};

    CHECK(tracer.value("a", 1u) == Status::Ok);
    CHECK(tracer.value("a", std::string(64, 'x')) == Status::TooLarge);
    CHECK(tracer.dropped() == 1);
    CHECK(!tracer.take());  // the open batch is kept
    CHECK(tracer.commit() == Status::Ok);
    CHECK(values(tracer.take()) == Values{1});
}

TEST_CASE("a message as large as a batch fits") {
    sonde::Storage<message_bytes, 2> storage;
    sonde::Tracer tracer{storage, nullptr};
    CHECK(tracer.value("a", 1u) == Status::Ok);
    CHECK(tracer.value("a", 2u) == Status::Ok);
    CHECK(tracer.value("a", 3u) == Status::Full);
    CHECK(values(tracer.take()) == Values{1});
}

TEST_CASE("the consumer requests a commit") {
    sonde::Storage<256, 2> storage;
    sonde::Tracer tracer{storage, nullptr};

    tracer.value("a", 1u);
    tracer.request_commit();
    CHECK(!tracer.take());  // the producer commits at its next message
    tracer.value("a", 2u);
    CHECK(values(tracer.take()) == Values{1, 2});
    tracer.release();

    tracer.value("a", 3u);  // the request is done
    CHECK(!tracer.take());

    tracer.request_commit();
    CHECK(tracer.commit() == Status::Ok);  // also does the request
    tracer.value("a", 4u);
    CHECK(values(tracer.take()) == Values{3});
    tracer.release();
    CHECK(!tracer.take());
}

TEST_CASE("the indices wrap around") {
    sonde::Storage<256, 3> storage;
    sonde::Tracer tracer{storage, nullptr};
    std::uint64_t next = 0;
    for (int round = 0; round < 100; ++round) {
        // a varying number of batches in flight
        const int batches = round % 3 + 1;
        for (int i = 0; i < batches; ++i) {
            tracer.value("a", next % 20);
            tracer.value("a", (next + 1) % 20);
            tracer.commit();
            next += 2;
        }
        CHECK(tracer.value("a", 0u) == (batches == 3 ? Status::Full : Status::Ok));
        if (batches < 3) {
            tracer.commit();
            ++next;
        }
        std::uint64_t expected = next - static_cast<std::uint64_t>(batches) * 2 -
                                 (batches < 3 ? 1 : 0);
        for (int i = 0; i < batches; ++i) {
            CHECK(values(tracer.take()) == Values{expected % 20, (expected + 1) % 20});
            tracer.release();
            expected += 2;
        }
        if (batches < 3) {
            CHECK(values(tracer.take()) == Values{0});
            tracer.release();
        }
        CHECK(!tracer.take());
    }
}

TEST_CASE("a sink takes the batches") {
    sonde::MemorySink sink;
    sonde::Storage<2 * message_bytes + 10, 1> storage;
    sonde::Tracer tracer{storage, sink, nullptr};

    tracer.value("a", 1u);
    tracer.value("a", 2u);
    CHECK(sink.bytes().empty());
    CHECK(tracer.value("a", 3u) == Status::Ok);  // writes the first batch
    CHECK(sink.bytes().size() == 2 * message_bytes);
    CHECK(tracer.commit() == Status::Ok);
    CHECK(sink.bytes().size() == 3 * message_bytes);
    CHECK(!tracer.take());  // nothing for a consumer

    const auto items = test::items(sink.bytes());
    REQUIRE(items);
    CHECK(items->size() == 3);

    tracer.value("a", 4u);
    tracer.request_commit();
    tracer.value("a", 5u);
    CHECK(sink.bytes().size() == 5 * message_bytes);
}

TEST_CASE("the messages of a batch the sink fails to take are dropped") {
    FailingSink sink;
    sonde::Storage<2 * message_bytes + 10, 1> storage;
    sonde::Tracer tracer{storage, sink, nullptr};

    tracer.value("a", 1u);
    tracer.value("a", 2u);
    CHECK(tracer.value("a", 3u) == Status::SinkError);
    CHECK(tracer.dropped() == 2);
    CHECK(tracer.commit() == Status::SinkError);
    CHECK(tracer.dropped() == 3);
    CHECK(tracer.commit() == Status::Ok);
}

// A static tracer is constant-initialized: its constructor works in a constant expression.
namespace {
sonde::Storage<64, 2> constant_storage;
[[maybe_unused]] constexpr sonde::Tracer constant_tracer{constant_storage, nullptr};
[[maybe_unused]] constexpr sonde::Tracer constant_tracer_with_default_clock{constant_storage};
}  // namespace

namespace changing {

/// Encodes as many elements as `first` the first time, then as `later`; a negative
/// count announces one element more than it adds.
int encodings = 0;

struct Samples {
    int first;
    int later;
};

void encode(sonde::Encoder& encoder, const Samples& samples) {
    const int count = encodings++ == 0 ? samples.first : samples.later;
    const auto elements = static_cast<std::size_t>(count < 0 ? -count : count);
    encoder.array(elements + (count < 0 ? 1 : 0), [&](sonde::Array& array) {
        for (std::size_t i = 0; i < elements; ++i) {
            array.add(1);
        }
    });
}

}  // namespace changing

TEST_CASE("a message that encodes differently in the next batch is checked again") {
    sonde::Storage<2 * message_bytes + 10, 2> storage;
    sonde::Tracer tracer{storage, nullptr};

    SECTION("too large the second time") {
        changing::encodings = 0;
        tracer.value("a", 1u);
        // 53 bytes, which do not fit after the first message, then 234 bytes
        CHECK(tracer.value("a", changing::Samples{20, 200}) == Status::TooLarge);
        CHECK(tracer.dropped() == 1);
    }
    SECTION("invalid the second time") {
        changing::encodings = 0;
        tracer.value("a", 1u);
        CHECK(tracer.value("a", changing::Samples{20, -2}) == Status::Invalid);
        CHECK(tracer.dropped() == 0);
    }

    CHECK(values(tracer.take()) == Values{1});
    tracer.release();
    CHECK(tracer.commit() == Status::Ok);  // the open batch is empty
    CHECK(!tracer.take());
    CHECK(tracer.value("a", 2u) == Status::Ok);
    CHECK(tracer.commit() == Status::Ok);
    CHECK(values(tracer.take()) == Values{2});
}

namespace {
int clock_reads = 0;
std::uint64_t counting_clock() {
    ++clock_reads;
    return 0;
}
}  // namespace

TEST_CASE("the clock is not read for a message without a batch") {
    sonde::Storage<message_bytes, 1> storage;
    sonde::Tracer tracer{storage, &counting_clock};
    clock_reads = 0;
    CHECK(tracer.value("a", 1u) == Status::Ok);
    CHECK(tracer.value("a", 2u) == Status::Full);  // commits the first batch
    CHECK(clock_reads == 2);
    CHECK(tracer.value("a", 3u) == Status::Full);
    CHECK(tracer.log(sonde::Level::Info, "", "x") == Status::Full);
    CHECK(clock_reads == 2);
    CHECK(tracer.value(sonde::Timestamp{0}, "a", 4u) == Status::Full);
    CHECK(clock_reads == 2);
}
