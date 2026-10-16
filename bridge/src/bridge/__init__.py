"""Bridge connecting a target to blip.

The public API, everything else is internal::

    from bridge import Protocol, action, ActionError, Range, Unit, LogLevel, run
"""

from bridge.actions import ActionError, Actions, Range, Unit, action
from bridge.main import run
from bridge.proto import Data, LogLevel, Message
from bridge.protocol import Protocol

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
