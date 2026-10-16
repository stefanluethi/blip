"""CLI main entry point for the Protocol to WebSocket Bridge."""

import asyncio
import logging
import signal
import sys
import click

from bridge.recording import RecordingWriter
from bridge.server import BridgeServer
from bridge.protocols.replay import ReplayProtocol
from bridge.protocols.robby import RobbyProtocol
from bridge.protocols.simulator import SimulatorProtocol
from bridge.protocols.socket import SocketProtocol


def setup_logging(verbose: bool) -> None:
    """Configure console logging level and format."""
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
        datefmt="%H:%M:%S"
    )


@click.command()
@click.option(
    "--protocol", "-p",
    type=click.Choice(["simulator", "tcp", "udp", "robby", "replay"], case_sensitive=False),
    default="simulator",
    help="Protocol mode to run (default: simulator)."
)
@click.option("--host", "-h", default="127.0.0.1", help="WebSocket server bind address.")
@click.option("--port", default=3000, type=int, help="WebSocket server port.")
@click.option("--path", default="/ws/bridge", help="WebSocket server endpoint path.")
@click.option("--remote-host", default="127.0.0.1", help="Remote host for TCP/UDP/robby mode.")
@click.option(
    "--remote-port", default=None, type=int,
    help="Remote port for TCP/UDP/robby mode (default: 9000, robby: 80)."
)
@click.option("--remote-path", default="/ws/robby", help="WebSocket endpoint path of robby.")
@click.option("--listen", is_flag=True, help="Listen mode for TCP/UDP sockets.")
@click.option("--interval", default=20, type=int, help="Simulator sampling interval in milliseconds.")
@click.option("--log-interval", default=500, type=int, help="Simulator mean log interval in milliseconds.")
@click.option(
    "--file", "replay_file", type=click.Path(exists=True, dir_okay=False),
    help="Recording file to replay in replay mode."
)
@click.option("--speed", default=1.0, type=float, help="Replay speed, 2 replays twice as fast.")
@click.option(
    "--record", type=click.Path(dir_okay=False),
    help="Record the messages sent to blip to this file (gzip-compressed CBOR sequence)."
)
@click.option("--verbose", "-v", is_flag=True, help="Enable verbose debug output.")
def main(
    protocol: str,
    host: str,
    port: int,
    path: str,
    remote_host: str,
    remote_port: int | None,
    remote_path: str,
    listen: bool,
    interval: int,
    log_interval: int,
    replay_file: str | None,
    speed: float,
    record: str | None,
    verbose: bool
) -> None:
    """Protocol to WebSocket Server Bridge CLI."""
    setup_logging(verbose)
    logger = logging.getLogger("bridge.main")
    logger.info(f"Initializing Bridge (Protocol: '{protocol}')")

    # Instantiate protocol backend
    if protocol.lower() == "simulator":
        proto_instance = SimulatorProtocol(interval_ms=interval, log_interval_ms=log_interval)
    elif protocol.lower() in ("tcp", "udp"):
        proto_instance = SocketProtocol(
            mode=protocol.lower(),
            remote_host=remote_host,
            remote_port=remote_port if remote_port is not None else 9000,
            listen=listen
        )
    elif protocol.lower() == "robby":
        proto_instance = RobbyProtocol(
            remote_host=remote_host,
            remote_port=remote_port if remote_port is not None else 80,
            remote_path=remote_path,
        )
    elif protocol.lower() == "replay":
        if replay_file is None:
            click.echo("Replay mode needs a recording file, pass it with --file", err=True)
            sys.exit(1)
        proto_instance = ReplayProtocol(path=replay_file, speed=speed)
    else:
        click.echo(f"Unsupported protocol: {protocol}", err=True)
        sys.exit(1)

    # Initialize server
    recorder = RecordingWriter(record) if record else None
    server = BridgeServer(
        protocol=proto_instance, host=host, port=port, path=path, recorder=recorder
    )

    async def async_main():
        await server.start()
        
        stop_event = asyncio.Event()
        loop = asyncio.get_running_loop()

        def signal_handler():
            logger.info("Termination signal received.")
            stop_event.set()

        for sig in (signal.SIGINT, signal.SIGTERM):
            try:
                loop.add_signal_handler(sig, signal_handler)
            except NotImplementedError:
                pass

        await stop_event.wait()
        await server.stop()

    try:
        asyncio.run(async_main())
    except KeyboardInterrupt:
        logger.info("Server stopped by keyboard interrupt.")
    finally:
        # also when stopped by Ctrl+C on Windows, where the server is not stopped
        if recorder:
            recorder.close()


if __name__ == "__main__":
    main()
