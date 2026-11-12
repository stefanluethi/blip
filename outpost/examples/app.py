"""A Python program that traces itself: a simulated room heater with a controller, each
with a tracer of its own, a dataclass record, a block of samples, Python's logging sent
to blip and an action.

    python examples/app.py

Connect blip to ws://127.0.0.1:3000/ws/outpost.
"""

import logging
import math
import random
import time
from dataclasses import dataclass
from typing import Annotated, List

import blip_outpost as outpost
from blip_outpost import Range, Unit

# the program's own logging, sent to blip at log/ and the logger's name
logger = logging.getLogger("heater.app")

# tracers by name, as loggers, e.g. at the top of each module
room = outpost.get_tracer("room")
heater = outpost.get_tracer("heater")
sensor = room.child("sensor")  # room/sensor


@dataclass
class HeaterState:
    """A record: each field a trace, heater/state.power and so on."""
    power: Annotated[float, Unit("W")]
    duty: Annotated[float, Unit("%")]
    on: bool


class Room:
    """A room that the heater warms and that loses heat to the outside."""

    def __init__(self) -> None:
        self.temperature = 18.0
        self.setpoint = 21.0

    def step(self, power_w: float, dt_s: float) -> None:
        loss = 0.02 * (self.temperature - 5.0)
        self.temperature += (power_w / 2000.0 - loss) * dt_s


class Controller:
    """A proportional controller of the heater."""

    def __init__(self, room: Room) -> None:
        self.room = room
        self.max_power_w = 1500.0
        self.on = False

    def step(self) -> float:
        error = self.room.setpoint - self.room.temperature
        duty = min(max(error * 50.0, 0.0), 100.0)
        on = duty > 0.0
        if on != self.on:
            self.on = on
            heater.info("heating" if on else "idle")
        power = self.max_power_w * duty / 100.0
        heater.value("state", HeaterState(power=power, duty=duty, on=on))
        if duty >= 100.0:
            heater.warn("at full power, %.1f °C below the setpoint", error)
        return power


the_room = Room()
controller = Controller(the_room)


@room.action("setpoint", readback="setpoint")
def set_setpoint(celsius: Annotated[float, Range(10.0, 30.0, step=0.5), Unit("°C")] = 21.0) -> None:
    """Room temperature setpoint"""
    the_room.setpoint = celsius
    room.value("setpoint", celsius)
    logger.info("setpoint set to %.1f °C", celsius)


def read_block(t: float, n: int = 50) -> List[float]:
    """Stands in for a block of samples of a fast sensor, e.g. read from a sound card."""
    return [0.2 * math.sin(2.0 * math.pi * 50.0 * (t + i / 1000.0)) + random.gauss(0.0, 0.01) for i in range(n)]


def main() -> None:
    logging.basicConfig(level=logging.INFO)
    logging.getLogger().addHandler(outpost.LogHandler())
    outpost.start()
    print("Serving blip on ws://127.0.0.1:3000/ws/outpost, Ctrl+C to stop")
    room.unit("temperature", "°C")
    room.unit("setpoint", "°C")
    sensor.unit("vibration", "g")
    room.value("setpoint", the_room.setpoint)
    logger.info("started")

    dt_s = 0.1
    start = time.monotonic()
    try:
        while True:
            # what one step sends is handed over at once, with one timestamp
            with outpost.batch():
                power = controller.step()
                the_room.step(power, dt_s)
                room.value("temperature", the_room.temperature)
            # 50 samples of the last 50 ms, 1 ms apart
            now_ns = time.time_ns()
            sensor.series("vibration", read_block(time.monotonic() - start), start_ns=now_ns - 50_000_000,
                          period_ns=1_000_000)
            time.sleep(dt_s)
    except KeyboardInterrupt:
        pass
    finally:
        outpost.stop()


if __name__ == "__main__":
    main()
