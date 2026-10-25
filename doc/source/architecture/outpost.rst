outpost
=======

The outpost is the Python package ``outpost`` (``outpost/src/outpost``),
managed with uv. It is a small framework: the protocol that talks to a target
is a class of the user, written against the public API exported by
``outpost/__init__.py``; everything else is internal. It runs on ``asyncio``
with the ``websockets`` and ``cbor2`` libraries.

Structure
---------

.. figure:: /diagrams/outpost-architecture.svg
   :target: ../_images/outpost-architecture.svg
   :class: diagram
   :alt: The CLI loads the protocol and creates it with its options; the
         protocol's run task sends messages to the server's broadcast, which
         writes the recording and queues the messages per blip client;
         invocations from blip are decoded, carried out by the protocol's
         actions and answered.

   Inside the outpost.

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Module
     - Responsibility
   * - ``main.py``
     - The ``outpost`` command line (``click``) and ``run``, which serves a
       protocol until interrupted: it starts the server, waits for a signal or
       the end of the protocol, and stops everything.
   * - ``loading.py``
     - ``load_protocol`` finds the protocol named by ``--protocol``: a
       built-in name, a ``.py`` file or an importable module, with
       ``:ClassName`` if it defines several. ``create_protocol`` converts the
       ``-o key=value`` options by the type hints of the constructor.
   * - ``protocol.py``
     - ``Protocol``, the base class: ``run``, ``on_client_connected``,
       ``invoke`` and the ``send_*`` helpers, and the ``actions`` registry,
       built from the decorated methods on first use.
   * - ``actions.py``
     - ``@action``, ``Range``, ``Unit``, ``ActionError`` and ``Actions``:
       derives the parameters of an action from the signature, converts and
       checks arguments, and runs the invocations of each action one after
       another behind a lock.
   * - ``server.py``
     - ``OutpostServer``: the WebSocket server, the broadcast to all clients,
       and carrying out invocations.
   * - ``proto.py``
     - The wire types mirrored from the Rust ``proto`` crate, CBOR encoding,
       splitting CBOR sequences and decoding requests.
   * - ``recording.py``
     - ``RecordingWriter`` and ``read_frames`` for recording files.
   * - ``protocols/``
     - The built-in ``simulator`` (also the reference protocol, using only the
       public API), ``replay`` and ``socket`` (TCP/UDP passthrough).

Message flow
------------

* The protocol's ``send`` encodes a ``Message`` (or splits bytes of complete
  messages, a CBOR sequence, dropping what is no CBOR) and calls the server's
  ``broadcast`` with one message at a time.
* ``broadcast`` drops ``ActionResult`` messages of the protocol (their ids are
  not those of any blip), remembers the latest ``Actions`` message for blips
  connecting later, writes the message to the recording, and queues it for
  each client.
* Each client has its own bounded queue (10 000 messages) and a sender task, so
  a slow blip neither holds up the protocol nor the other blips; when its queue
  is full, messages for it are dropped and counted in the log. The sender sends
  what is queued together, as a CBOR sequence in frames of up to 64 KiB, so a
  busy target does not cost a frame per message.
* A connecting client is checked against the path, gets the cached actions
  first, then everything broadcast from then on; then
  ``on_client_connected`` is called.

Invocations
-----------

Each binary frame from a blip is decoded as a ``Request``. The invocation runs
in a task of its own, so a slow action does not hold up the others:

#. ``protocol.invoke(path, args)``; by default ``Actions.invoke``, which looks
   the action up, rejects it if unknown or disabled, converts and checks each
   argument against its parameter, and calls the function under the action's
   lock -- the invocations of one action run one after another, in order.
#. The answer, ``ActionResult`` with ``Ok`` or ``Err`` and the reason, goes into
   the queue of the invoking client, in order with the broadcast messages.
#. A log record of the component ``actions`` (``DEBUG`` if accepted, ``WARN``
   if rejected) is broadcast to all clients and the recording.

The ``tcp`` and ``udp`` protocols override ``invoke`` to forward an ``Invoke``
request with an id of their own to the target and accept once it is sent.

Changes to ``protocol.actions`` (add, remove, enable, disable) call back into
the protocol, which announces the whole list again.

Socket passthrough
------------------

``SocketProtocol`` connects to the target or listens for it. Over TCP it reads
the stream in chunks, keeps the start of an incomplete message until the rest
arrives (at most 16 MiB) and forwards each complete message; data that is no
CBOR is dropped and reading starts afresh with the next chunk. Over UDP each
datagram is a CBOR sequence of complete messages.

Recording and replay
--------------------

``RecordingWriter`` writes the version 1 header and then each broadcast
message to a gzip file, flushing every second so the file is readable up to
then also if the process is killed. ``read_frames`` reads version 1 and 2,
compressed or not, several gzip members, and stops at a cut-off message. The
replay sends the frames at the pace of their timestamps (scaled by ``speed``)
and rewrites ``Actions`` messages to disabled ones.

Tests
-----

``outpost/tests`` covers the actions registry, the loading of protocols, the
server (broadcast, ordering, answering, slow clients), the socket
passthrough and the recordings, with ``pytest`` and ``pytest-asyncio``::

   cd outpost && uv run pytest
