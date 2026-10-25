"""Helpers for protocols that send at a steady pace."""

import asyncio
import time


async def sleep_until(deadline: float) -> None:
    """Sleeps until `time.perf_counter()` reaches `deadline`. On Windows the event loop
    runs timers up to its clock resolution of about 16 ms early, so a single sleep of
    10 ms may return right away."""
    while (remaining := deadline - time.perf_counter()) > 0:
        await asyncio.sleep(remaining)
