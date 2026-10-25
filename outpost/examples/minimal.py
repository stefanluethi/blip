"""The smallest script sending to blip: a made-up sensor read in a loop, a log record
now and then, and an action that changes what it sends.

    python examples/minimal.py

Connect blip to ws://127.0.0.1:3000/ws/outpost. A script for a real target reads and
decodes what the target sends in place of `read_sensor`.
"""

import math
import random
import time
from typing import Annotated

import blip_outpost as outpost
from blip_outpost import LogLevel, Range, Unit

amplitude = 1.0


def read_sensor(t: float) -> float:
    """Stands in for reading the target, e.g. a line from a serial port."""
    return amplitude * math.sin(2.0 * math.pi * 0.5 * t) + random.gauss(0.0, 0.02)


@outpost.action("sensor/amplitude", readback="sensor/amplitude")
def set_amplitude(volts: Annotated[float, Range(0.0, 5.0, step=0.1), Unit("V")] = 1.0) -> None:
    """Amplitude of the sensor signal"""
    global amplitude
    amplitude = volts
    outpost.send_value("sensor/amplitude", volts)
    outpost.send_log(LogLevel.INFO, f"amplitude set to {volts} V", component="sensor")


def main() -> None:
    outpost.start()
    print("Serving blip on ws://127.0.0.1:3000/ws/outpost, Ctrl+C to stop")
    outpost.send_metadata("sensor/voltage", "V")
    outpost.send_metadata("sensor/amplitude", "V")
    outpost.send_value("sensor/amplitude", amplitude)

    start = time.monotonic()
    count = 0
    try:
        while True:
            outpost.send_value("sensor/voltage", read_sensor(time.monotonic() - start))
            count += 1
            if count % 100 == 0:
                outpost.send_log(LogLevel.DEBUG, f"{count} samples sent", component="main")
            time.sleep(0.01)
    except KeyboardInterrupt:
        pass
    finally:
        outpost.stop()


if __name__ == "__main__":
    main()
