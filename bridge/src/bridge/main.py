"""CLI main entry point for the Protocol to WebSocket Bridge."""

import asyncio
import logging
import signal
import sys
import click

from bridge.server import BridgeServer
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
    type=click.Choice(["simulator", "tcp", "udp"], case_sensitive=False),
    default="simulator",
    help="Protocol mode to run (default: simulator)."
)
@click.option("--host", "-h", default="127.0.0.1", help="WebSocket server bind address.")
@click.option("--port", default=3000, type=int, help="WebSocket server port.")
@click.option("--path", default="/ws/bridge", help="WebSocket server endpoint path.")
@click.option("--remote-host", default="127.0.0.1", help="Remote host for TCP/UDP mode.")
@click.option("--remote-port", default=9000, type=int, help="Remote port for TCP/UDP mode.")
@click.option("--listen", is_flag=True, help="Listen mode for TCP/UDP sockets.")
@click.option("--interval", default=20, type=int, help="Simulator sampling interval in milliseconds.")
@click.option("--log-interval", default=500, type=int, help="Simulator mean log interval in milliseconds.")
@click.option("--verbose", "-v", is_flag=True, help="Enable verbose debug output.")
def main(
    protocol: str,
    host: str,
    port: int,
    path: str,
    remote_host: str,
    remote_port: int,
    listen: bool,
    interval: int,
    log_interval: int,
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
            remote_port=remote_port,
            listen=listen
        )
    else:
        click.echo(f"Unsupported protocol: {protocol}", err=True)
        sys.exit(1)

    # Initialize server
    server = BridgeServer(protocol=proto_instance, host=host, port=port, path=path)

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


if __name__ == "__main__":
    main()
