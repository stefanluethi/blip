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
from blip_outpost.protolink import ProtobufLink
from blip_outpost.replay import Replay
from blip_outpost.schema import Schema, SchemaError

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
    """Serves blip the messages of a target that speaks the blip protocol itself or
    protobuf, or of a recording. To connect a target of your own, use blip_outpost in a
    script."""


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
@click.argument("url")
@click.option("--proto", "protos", multiple=True, required=True,
              type=click.Path(exists=True, dir_okay=False, path_type=Path),
              help="The schema, a .proto file; may be given several times.")
@click.option("-I", "--include", "includes", multiple=True,
              type=click.Path(exists=True, file_okay=False, path_type=Path),
              help="Directory to look for imports in (protoc -I); may be given several times.")
@click.option("--uplink", required=True,
              help="Full name of the message type the target sends, e.g. robby.link.Uplink.")
@click.option("--downlink",
              help="Full name of the message type the target receives; its actions are offered to blip.")
@click.option("--timeout", default=1.0, show_default=True,
              help="Seconds to wait for the answer of the target to an action.")
@click.option("--protoc", default="protoc", show_default=True, help="The protoc to compile the schema with.")
@click.option("--idle-timeout", default=5.0, show_default=True,
              help="Seconds without a message after which the target counts as lost and the "
                   "outpost connects again; 0 waits forever.")
@outpost_options
def protobuf(
    url: str,
    protos: tuple,
    includes: tuple,
    uplink: str,
    downlink: Optional[str],
    timeout: float,
    protoc: str,
    idle_timeout: float,
    **options: Any,
) -> None:
    """Translates the protobuf messages of a target at the WebSocket URL to blip.

    The schema is loaded at run time; its annotations (blip/options.proto) say which
    trace, unit and scale each field becomes, and which actions the target offers."""
    outpost = create_outpost(**options)
    try:
        schema = Schema.load(protos, includes, protoc)
        link = ProtobufLink(outpost, schema, url, uplink, downlink, timeout=timeout, idle_timeout=idle_timeout)
    except SchemaError as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)
    serve(outpost, link.run)


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
