// Producer and consumer on two threads, as on a target with a task that traces and one
// that sends. Run it under ThreadSanitizer, too.

#include "cbor.hpp"

#include <catch2/catch_test_macros.hpp>

#include <atomic>
#include <thread>

using sonde::Status;

TEST_CASE("a producer and a consumer on two threads") {
    constexpr std::uint64_t messages = 200'000;
    sonde::Storage<256, 4> storage;
    sonde::Tracer tracer{storage, nullptr};
    std::atomic<bool> done{false};

    std::uint64_t traced = 0;
    std::uint64_t full = 0;
    std::thread producer{[&] {
        for (std::uint64_t i = 0; i < messages; ++i) {
            const Status status = tracer.value("n", i);
            if (status == Status::Ok) {
                ++traced;
            } else if (status == Status::Full) {
                ++full;
            }
            if (i % 16 == 0) {
                tracer.commit();
            }
        }
        tracer.commit();
        done.store(true, std::memory_order_release);
    }};

    std::uint64_t taken = 0;
    std::uint64_t last = 0;
    bool decoded = true;
    bool ascending = true;
    std::thread consumer{[&] {
        for (;;) {
            // after done, the remaining batches are all ready
            const bool finished = done.load(std::memory_order_acquire);
            while (const sonde::Batch batch = tracer.take()) {
                const auto items = test::items(batch.data, batch.size);
                decoded = decoded && items.has_value();
                if (items) {
                    for (const auto& item : *items) {
                        const auto value = test::uint_value(item);
                        decoded = decoded && value.has_value();
                        if (value) {
                            ascending = ascending && (taken == 0 || *value > last);
                            last = *value;
                            ++taken;
                        }
                    }
                }
                tracer.release();
            }
            if (finished) {
                break;
            }
            tracer.request_commit();
            std::this_thread::yield();
        }
    }};

    producer.join();
    consumer.join();

    CHECK(decoded);
    CHECK(ascending);
    CHECK(traced + full == messages);
    CHECK(taken == traced);
    CHECK(tracer.dropped() == full);
}
