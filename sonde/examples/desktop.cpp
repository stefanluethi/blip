// Traces a damped oscillation to a recording file, to open in blip.
//
//     sonde_example_desktop [path]    (default: oscillation.blip)

#include <sonde/file_sink.hpp>

#include <cmath>
#include <cstdio>

int main(int argc, char** argv) {
    const char* path = argc > 1 ? argv[1] : "oscillation.blip";
    sonde::FileSink file{path};
    if (!file.is_open()) {
        std::fprintf(stderr, "cannot write %s\n", path);
        return 1;
    }
    sonde::Storage<4096, 1> storage;
    sonde::Tracer tracer{storage, file};  // UTC from the system clock

    // simulated time, 1 ms per step from now on
    const std::uint64_t start = sonde::default_clock()();
    tracer.metadata("oscillator/position", "m");
    tracer.metadata("oscillator/velocity", "m/s");
    tracer.log(sonde::Timestamp{start}, sonde::Level::Info, "oscillator", "started");

    double position = 1.0;
    double velocity = 0.0;
    for (int step = 0; step < 10'000; ++step) {
        const double dt = 0.001;
        velocity += (-40.0 * position - 0.5 * velocity) * dt;
        position += velocity * dt;

        const sonde::Timestamp now{start + static_cast<std::uint64_t>(step) * 1'000'000};
        tracer.value(now, "oscillator/position", position);
        tracer.value(now, "oscillator/velocity", velocity);
        tracer.value(now, "oscillator/state", sonde::record(2, [&](sonde::Record& record) {
                         record.field("energy", 20.0 * position * position +
                                                    0.5 * velocity * velocity);
                         record.field("moving_up", velocity > 0);
                     }));
        if (step % 1000 == 0) {
            char message[64];
            std::snprintf(message, sizeof message, "step %d: x = %.3f", step, position);
            tracer.log(now, sonde::Level::Debug, "oscillator", message);
        }
    }
    tracer.commit();  // before the file closes
    std::printf("wrote %s, %u messages dropped\n", path, static_cast<unsigned>(tracer.dropped()));
    return 0;
}
