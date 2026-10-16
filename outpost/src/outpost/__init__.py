"""Outpost connecting a target to blip.

The public API, everything else is internal::

    from outpost import Protocol, action, ActionError, Range, Unit, LogLevel, run
"""

from outpost.actions import ActionError, Actions, Range, Unit, action
from outpost.main import run
from outpost.proto import Data, LogLevel, Message
from outpost.protocol import Protocol

__version__ = "0.1.0"

__all__ = [
    "ActionError",
    "Actions",
    "Data",
    "LogLevel",
    "Message",
    "Protocol",
    "Range",
    "Unit",
    "action",
    "run",
]
