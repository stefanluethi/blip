"""The command line of the outpost: the passthrough of targets that speak the blip
protocol themselves, and the replay of recordings. Your own targets are served by a
script of yours using the library."""

import asyncio
import logging
import signal
import sys
from pathlib import Path
from typing import Any, Awaitable, Callable, Optional

import click

from blip_outpost.outpost import Outpost
from blip_outpost.passthrough import SocketPassthrough
from blip_outpost.replay import Replay

logger = logging.getLogger(__name__)


def setup_logging(verbose: bool) -> None:
    """Configure console logging level and format."""
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
        datefmt="%H:%M:%S"
    )


def serve(outpost: Outpost, task: Callable[[], Awaitable[None]]) -> None:
    """Serves blip with `outpost` while `task` runs, until interrupted. If the task
    ends, e.g. a replay, blip still sees what it sent until interrupted; if it raises,
    the outpost stops with its error."""

    async def main() -> None:
        async with outpost:
            stop = asyncio.Event()
            loop = asyncio.get_running_loop()
            for sig in (signal.SIGINT, signal.SIGTERM):
                try:
                    loop.add_signal_handler(sig, stop.set)
                except NotImplementedError:
                    pass
            stopped = asyncio.create_task(stop.wait())
            running = asyncio.create_task(task())
            await asyncio.wait([stopped, running], return_when=asyncio.FIRST_COMPLETED)
            if running.done() and not running.cancelled() and running.exception() is None:
                logger.info("Done, serving until interrupted")
                await stopped
            stopped.cancel()
            running.cancel()
            try:
                await running
            except asyncio.CancelledError:
                pass

    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        logger.info("Stopped")
    except Exception as e:
        logger.error(f"Stopped: {type(e).__name__}: {e}")
        sys.exit(1)


def outpost_options(command: Callable[..., Any]) -> Callable[..., Any]:
    """The options of the WebSocket server blip connects to, shared by all commands."""
    options = [
        click.option("--ws-host", default="127.0.0.1", show_default=True,
                     help="Address the WebSocket server binds to; 0.0.0.0 serves other machines too."),
        click.option("--ws-port", default=3000, type=int, show_default=True,
                     help="WebSocket server port."),
        click.option("--ws-path", default="/ws/outpost", show_default=True,
                     help="WebSocket endpoint path."),
        click.option("--record", type=click.Path(dir_okay=False),
                     help="Record the messages sent to blip to this file."),
        click.option("--verbose", "-v", is_flag=True, help="Debug output."),
    ]
    for option in reversed(options):
        command = option(command)
    return command


def create_outpost(ws_host: str, ws_port: int, ws_path: str, record: Optional[str], verbose: bool) -> Outpost:
    setup_logging(verbose)
    return Outpost(host=ws_host, port=ws_port, path=ws_path, record=record)


@click.group()
def main() -> None:
    """Serves blip the messages of a target that speaks the blip protocol itself, or of
    a recording. To connect a target of your own, use blip_outpost in a script."""


@main.command()
@click.argument("host")
@click.argument("port", type=int)
@click.option("--listen", is_flag=True, help="Wait for the target to connect at HOST:PORT instead.")
@outpost_options
def tcp(host: str, port: int, listen: bool, **options: Any) -> None:
    """Passes through the messages of a target at HOST:PORT over TCP."""
    outpost = create_outpost(**options)
    serve(outpost, SocketPassthrough(outpost, "tcp", host, port, listen).run)


@main.command()
@click.argument("host")
@click.argument("port", type=int)
@click.option("--listen", is_flag=True, help="Receive the datagrams of the target at HOST:PORT instead.")
@outpost_options
def udp(host: str, port: int, listen: bool, **options: Any) -> None:
    """Passes through the messages of a target at HOST:PORT over UDP."""
    outpost = create_outpost(**options)
    serve(outpost, SocketPassthrough(outpost, "udp", host, port, listen).run)


@main.command()
@click.argument("file", type=click.Path(exists=True, dir_okay=False, path_type=Path))
@click.option("--speed", default=1.0, show_default=True, help="2 replays twice as fast.")
@outpost_options
def replay(file: Path, speed: float, **options: Any) -> None:
    """Replays a recording FILE at the pace of its timestamps, once blip connects."""
    outpost = create_outpost(**options)
    try:
        replaying = Replay(outpost, file, speed)
    except ValueError as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)
    serve(outpost, replaying.run)


if __name__ == "__main__":
    main()
