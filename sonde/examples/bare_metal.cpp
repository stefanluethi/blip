// A sketch of a firmware: the control task traces, the network task sends one UDP
// datagram per batch, and an ADC interrupt traces with its own tracer, sent over a UART
// by DMA. The hardware access is declared, not defined, so this only compiles.

#include <sonde/sonde.hpp>

#include <cstdint>

// --- The firmware's platform --------------------------------------------------------

std::uint64_t uptime_ns();
void wait_for_cycle();
void sleep_ms(std::uint32_t ms);
double read_speed();
double read_current();
std::uint16_t read_adc();
void udp_send(const std::uint8_t* data, std::size_t size);
bool uart_dma_busy();
void uart_dma_send_cobs(const std::uint8_t* data, std::size_t size);

struct Pid {
    float setpoint;
    float error;
    float output;
};

void encode(sonde::Encoder& encoder, const Pid& pid) {
    encoder.record(3, [&](sonde::Record& record) {
        record.field("setpoint", pid.setpoint);
        record.field("error", pid.error);
        record.field("output", pid.output);
    });
}

// --- Trace in the control task, send in the network task ----------------------------

namespace {
// one UDP datagram per batch, the batch size fits the Ethernet MTU
sonde::Storage<1400, 4> control_storage;
sonde::Tracer control_traces{control_storage, &uptime_ns};
Pid pid_state{};
}  // namespace

void control_task() {  // producer
    control_traces.metadata("motor/speed", "rpm");
    control_traces.metadata("motor/current", "A");
    for (;;) {
        wait_for_cycle();
        control_traces.value("motor/speed", read_speed());
        control_traces.value("motor/current", read_current());
        control_traces.value("pid", pid_state);
        control_traces.commit();  // one batch per cycle at most
    }
}

void network_task() {  // consumer
    for (;;) {
        while (auto batch = control_traces.take()) {
            udp_send(batch.data, batch.size);
            control_traces.release();
        }
        sleep_ms(5);
    }
}

// --- Trace in an ISR, send by DMA over a UART ---------------------------------------

namespace {
sonde::Storage<256, 3> adc_storage;
sonde::Tracer adc_traces{adc_storage, &uptime_ns};
}  // namespace

extern "C" void ADC_IRQHandler() {  // producer
    adc_traces.value("adc/raw", read_adc());
}

void comm_task() {  // consumer
    for (;;) {
        adc_traces.request_commit();  // the ISR commits at its next sample
        if (!uart_dma_busy()) {
            if (auto batch = adc_traces.take()) {
                uart_dma_send_cobs(batch.data, batch.size);  // framing on the UART
            }
        }
        sleep_ms(10);
    }
}

extern "C" void DMA_TxComplete_IRQHandler() {
    adc_traces.release();
}
