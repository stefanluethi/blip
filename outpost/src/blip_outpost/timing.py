"""Helpers for protocols that send at a steady pace, and for the time of a target."""

import asyncio
import logging
import time
from typing import Any, Callable, Optional, Tuple

logger = logging.getLogger(__name__)

# A timestamp this far behind the newest one of the target means the target restarted
RESTART_JUMP_NS = 1_000_000_000
# After a new connection, a timestamp this far behind the newest one is a restart: less
# is a sample that was late, e.g. buffered while the connection was lost
RECONNECT_JUMP_NS = 100_000_000
# A timestamp at least this far behind the newest one, and less than half of it, is a
# restart of a target that ran shortly, e.g. in a crash loop
SHORT_RUN_JUMP_NS = 10_000_000


async def sleep_until(deadline: float) -> None:
    """Sleeps until `time.perf_counter()` reaches `deadline`. On Windows the event loop
    runs timers up to its clock resolution of about 16 ms early, so a single sleep of
    10 ms may return right away."""
    while (remaining := deadline - time.perf_counter()) > 0:
        await asyncio.sleep(remaining)


class TargetClock:
    """Keeps the time of a target going forward when the target restarts and its clock
    starts over, e.g. its uptime at 0, so that blip does not put the samples of the new
    run among those of the old one.

    `map` turns a timestamp of the target into the one sent to blip: the timestamp plus
    an offset, 0 until the first restart. A timestamp behind the newest one is a restart
    if it is
    - more than `RESTART_JUMP_NS` behind,
    - at least `SHORT_RUN_JUMP_NS` behind and less than half the newest, as after a run
      shorter than a second,
    - more than `RECONNECT_JUMP_NS` behind after `restart()`, e.g. after a lost
      connection, or any amount behind after `restart(sure=True)`, e.g. after a
      sequence number that started over.

    The new run then continues at the newest time sent plus the time since it arrived,
    measured with `clock`, and `on_restart(time_ns)` is called with the time it
    continues at.

    Also estimates the time of the target now, see `now`."""

    def __init__(
        self,
        clock: Callable[[], int] = time.monotonic_ns,
        on_restart: Optional[Callable[[int], None]] = None,
    ) -> None:
        self.offset = 0
        self.restarts = 0
        self.on_restart = on_restart
        self._clock = clock
        # the newest timestamp of the target, the one sent for it, and the clock then
        self._newest: Optional[Tuple[int, int, int]] = None
        # how far behind the newest the next timestamp may be without a restart
        self._next_jump_ns = RESTART_JUMP_NS

    @classmethod
    def reporting_to(cls, outpost: Any, clock: Callable[[], int] = time.monotonic_ns) -> "TargetClock":
        """A clock that tells the blips of `outpost` when the target restarts, see
        `report_restart`."""
        target = cls(clock)
        target.on_restart = lambda t: report_restart(outpost, target, t)
        return target

    @property
    def known(self) -> bool:
        """Whether a timestamp of the target arrived."""
        return self._newest is not None

    def restart(self, sure: bool = False) -> None:
        """The target may have restarted, or with `sure` it did: an earlier timestamp
        next is a restart if it is more than `RECONNECT_JUMP_NS` earlier, with `sure`
        any earlier one."""
        jump = 0 if sure else RECONNECT_JUMP_NS
        self._next_jump_ns = min(self._next_jump_ns, jump)

    def map(self, t: int) -> int:
        """The timestamp to send for the timestamp `t` of the target, in ns."""
        newest = self._newest
        restarted = False
        if newest is not None:
            target, sent, at = newest
            back = target - t
            if back > self._next_jump_ns or (back >= SHORT_RUN_JUMP_NS and t < target // 2):
                # at least 1 ns after the newest, should the clock not have moved
                self.offset = sent + max(self._clock() - at, 1) - t
                self.restarts += 1
                restarted = True
        self._next_jump_ns = RESTART_JUMP_NS
        mapped = t + self.offset
        self.seen(mapped)
        if restarted and self.on_restart is not None:
            self.on_restart(mapped)
        return mapped

    def shift(self, t: int) -> int:
        """The timestamp `t` in the time of the target shifted by the restarts so far,
        as `map` would, without taking it for the newest or a restart, e.g. for a
        timestamp of another clock that follows the target's."""
        return t + self.offset

    def seen(self, mapped: int) -> None:
        """A sample at the time `mapped`, as `map` returns it, arrived: the estimate of
        `now` counts from it if it is the newest."""
        if self._newest is None or mapped >= self._newest[1]:
            self._newest = (mapped - self.offset, mapped, self._clock())

    def now(self) -> Optional[int]:
        """The estimated time of the target now, as `map` returns it: its newest
        timestamp plus the time since; `None` before the first."""
        if self._newest is None:
            return None
        _, sent, at = self._newest
        return sent + (self._clock() - at)


def report_restart(outpost: Any, clock: TargetClock, mapped: int) -> None:
    """Tells blip with a log record at `mapped`, and the log of the outpost, that the
    target of `clock` restarted."""
    text = (f"The target restarted: its timestamps are shifted by {clock.offset / 1e9:.3f} s "
            "from now on, to follow the ones before")
    logger.warning(text)
    # a warning of the target as a whole
    outpost.send_log(3, text, "outpost", mapped)
