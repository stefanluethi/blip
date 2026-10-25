"""The command line of the outpost, and `run` to serve a protocol."""

import asyncio
import logging
import signal
import sys
from pathlib import Path
from typing import Optional, Tuple, Union

import click

from outpost.loading import LoadError, create_protocol, describe_options, load_protocol
from outpost.protocol import Protocol
from outpost.recording import RecordingWriter
from outpost.server import OutpostServer

logger = logging.getLogger("outpost.main")


def setup_logging(verbose: bool) -> None:
    """Configure console logging level and format."""
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
        datefmt="%H:%M:%S"
    )


def run(
    protocol: Protocol,
    *,
    host: str = "127.0.0.1",
    port: int = 3000,
    path: str = "/ws/outpost",
    record: Optional[Union[str, Path]] = None,
) -> None:
    """Serves `protocol` to blip on ws://host:port/path until interrupted, recording
    the messages sent to blip to the file `record` if given. Raises the exception the
    protocol stopped with, if any."""
    recorder = RecordingWriter(record) if record else None
    server = OutpostServer(protocol=protocol, host=host, port=port, path=path, recorder=recorder)

    async def serve() -> None:
        await server.start()
        stop = asyncio.Event()
        loop = asyncio.get_running_loop()
        for sig in (signal.SIGINT, signal.SIGTERM):
            try:
                loop.add_signal_handler(sig, stop.set)
            except NotImplementedError:
                pass

        stopped = asyncio.create_task(stop.wait())
        task = server.protocol_task
        assert task is not None
        await asyncio.wait([stopped, task], return_when=asyncio.FIRST_COMPLETED)
        if task.done() and not task.cancelled() and task.exception() is None:
            # the protocol is done, e.g. a replay, blip still sees what it sent
            logger.info("The protocol finished, serving until interrupted")
            await stopped
        stopped.cancel()
        await server.stop()
        if task.done() and not task.cancelled() and task.exception() is not None:
            raise task.exception()  # type: ignore[misc]

    try:
        asyncio.run(serve())
    except KeyboardInterrupt:
        logger.info("Server stopped by keyboard interrupt.")
    finally:
        # also when stopped by Ctrl+C on Windows, where the server is not stopped
        if recorder:
            recorder.close()


@click.command()
@click.option(
    "--protocol", "-p", default="simulator",
    help="The protocol: simulator, fast, replay, tcp, udp, or a Python file or module of a "
         "protocol, followed by :ClassName if it defines several (default: simulator).",
)
@click.option(
    "--option", "-o", "options", multiple=True, metavar="KEY=VALUE",
    help="An option of the protocol, repeat for several. See --protocol-help.",
)
@click.option("--protocol-help", is_flag=True, help="Show the options of the protocol and exit.")
@click.option("--host", "-h", default="127.0.0.1", help="WebSocket server bind address.")
@click.option("--port", default=3000, type=int, help="WebSocket server port.")
@click.option("--path", default="/ws/outpost", help="WebSocket server endpoint path.")
@click.option(
    "--record", type=click.Path(dir_okay=False),
    help="Record the messages sent to blip to this file (gzip-compressed CBOR sequence)."
)
@click.option("--verbose", "-v", is_flag=True, help="Enable verbose debug output.")
def main(
    protocol: str,
    options: Tuple[str, ...],
    protocol_help: bool,
    host: str,
    port: int,
    path: str,
    record: Optional[str],
    verbose: bool,
) -> None:
    """Serves a protocol connecting a target to blip over WebSocket."""
    setup_logging(verbose)
    try:
        factory = load_protocol(protocol)
        if protocol_help:
            click.echo(f"Options of {protocol}:\n{describe_options(factory)}")
            return
        instance = create_protocol(factory, options)
    except LoadError as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)

    logger.info(f"Initializing Outpost (Protocol: '{protocol}')")
    try:
        run(instance, host=host, port=port, path=path, record=record)
    except Exception as e:
        logger.error(f"The protocol stopped: {type(e).__name__}: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
