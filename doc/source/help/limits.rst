Limits and safety
=================

What blip, the outpost and sonde do not do, how far they go, and what to take
care of.

Safety
------

.. warning::

   blip carries out actions on real hardware. Anyone who can reach an outpost
   can invoke its actions.

No authentication, no encryption
   An outpost accepts every blip that connects, and serves ``ws://``, which is
   not encrypted. Every connected blip receives all messages and can invoke
   all actions.

Served to the machine only, by default
   The outpost binds to ``127.0.0.1``: only blips on the same machine reach
   it. ``--ws-host 0.0.0.0`` (or ``start(host="0.0.0.0")``) serves everyone
   who can reach the machine on the network. Do so only on a network you
   trust, or put the outpost behind a proxy that serves ``wss://`` and checks
   who connects.

Actions act right away
   An action is carried out when it is invoked; a dragged slider sends up to
   ten times a second. Mark actions that must not happen by mistake with
   ``confirm=True`` (``confirm`` in a protobuf schema), so blip asks first. The
   outpost checks the arguments against the parameters of the action, but a
   target that passes on its actions with ``tcp`` or ``udp`` gets them
   unchecked, so it checks them itself. See :ref:`outpost-actions`.

blip never acts on its own
   blip invokes an action only when you do: not at startup, not when it
   connects again, and only while recording. A replay carries out no actions.

Limits
------

Platforms
   The web app runs in current desktop browsers; the hosted one, served over
   ``https``, connects only to outposts on the same machine, see
   :doc:`/start/installation`. The native app is built from source with Rust. The
   outpost needs Python 3.10 or newer; sonde a C++17 compiler.

Message rates
   An outpost sends at most about 200 000 messages per second, as far as
   Python gets; connect blip to several outposts for more. How fast blip takes
   messages in depends on the machine and on what the views show: measure it
   with the ``fast`` example, see :doc:`/outpost/index`.

Queues
   An outpost queues up to 10 000 messages for each blip and drops messages
   for a blip that falls further behind, with a warning. ``drain()`` slows
   the script to the slowest blip instead.

Recordings
   blip keeps a recording in memory while it records and while it is open, so
   its length is limited by the memory of the machine, and in the browser by
   what the browser gives a page. Save long recordings and start afresh.

Values
   The first value of a path sets its kind, and the first record its fields;
   other values at the path are dropped, see :doc:`/help/troubleshooting`. A value
   older than the newest 65 536 of its path is dropped too.

Timestamps
   Nanoseconds, either since 1970 (UTC) or since the target booted, as the
   ``Hello`` says, else guessed. Targets with different time bases line up
   only as far as their clocks agree; with both declared, the time of day wins.

Protocol version
   blip reads protocol version 1. A target that sends a newer ``Hello`` is
   read on, and blip warns.

Actions
   An invocation the target does not answer within five seconds counts as
   unanswered. sonde cannot offer actions yet; a target with sonde offers
   them through its own outpost script.

Data that is not kept
   blip keeps values and log records only while recording; before, it
   collects only the trace metadata and actions the targets announce. Invocations are
   not recorded; the log records of the target show what it did.
