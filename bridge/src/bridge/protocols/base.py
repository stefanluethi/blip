"""Abstract Base Protocol Class."""

from abc import ABC, abstractmethod
from typing import Callable, Awaitable, Optional, Union
import logging

logger = logging.getLogger("bridge.protocol")

BroadcastCallback = Callable[[Union[bytes, str]], Awaitable[None]]


class BaseProtocol(ABC):
    """Abstract Base Class for protocol handlers."""

    def __init__(self) -> None:
        self.broadcast_callback: Optional[BroadcastCallback] = None

    def register_broadcast_callback(self, callback: BroadcastCallback) -> None:
        """Register callback function used to push messages to connected WebSocket clients."""
        self.broadcast_callback = callback

    async def send_to_websocket(self, message: Union[bytes, str]) -> None:
        """Forward message from protocol driver to WebSocket server clients."""
        if self.broadcast_callback:
            try:
                await self.broadcast_callback(message)
            except Exception as e:
                logger.error(f"Error in broadcast callback: {e}")

    @abstractmethod
    async def start(self) -> None:
        """Start protocol task / connection."""
        pass

    @abstractmethod
    async def stop(self) -> None:
        """Stop protocol task / connection."""
        pass

    @abstractmethod
    async def handle_incoming_ws_message(self, message: Union[bytes, str]) -> None:
        """Handle or forward incoming message sent from WebSocket client to the protocol backend."""
        pass
