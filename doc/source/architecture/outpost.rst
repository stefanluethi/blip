outpost
=======

The outpost is the Python library ``blip-outpost``, imported as
``blip_outpost`` (``outpost/src/blip_outpost``), managed with uv. The user's
script owns the link to the target and calls the library: it sends what it
decodes and declares its actions as functions. The public API is exported by
``blip_outpost/__init__.py``; everything else is internal. The outpost runs on
``asyncio`` with the ``websockets`` and ``cbor2`` libraries, in a thread of its
own or in the event loop of the script. The examples live next to it in
``outpost/examples`` and are not part of the library.

Structure
---------

.. figure:: /diagrams/outpost-architecture.svg
   :target: ../_images/outpost-architecture.svg
   :class: diagram
   :alt: The user's script, or the command line with the passthrough and the
         replay, talks to the target and uses the Outpost; its send functions
         pass messages to the server's broadcast, which writes the recording
         and queues the messages per blip client; invocations from blip are
         decoded, carried out by the outpost's actions and answered.

   Inside the outpost.

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Module
     - Responsibility
   * - ``__init__.py``
     - The public API, and functions that use one ``Outpost`` of the program:
       ``start``, ``send_value``, ``action``, … for scripts that need only one.
   * - ``outpost.py``
     - ``Outpost``: serves from a thread of its own (``start``) or from the
       running event loop (``async with``); ``send_*`` from any thread; the
       ``actions`` and the ``action`` decorator and ``register``; ``drain`` and
       ``wait_for_client``; ``on_invoke`` to carry out invocations in place of
       the actions.
   * - ``actions.py``
     - ``Range``, ``Unit``, ``ActionError`` and ``Actions``: derives the
       parameters of an action from the signature, converts and checks
       arguments, and runs the invocations of each action one after another
       behind a lock.
   * - ``server.py``
     - ``OutpostServer``: the WebSocket server, the broadcast to all clients,
       the cache of units and actions, and carrying out invocations.
   * - ``proto.py``
     - The wire types mirrored from the Rust ``proto`` crate, CBOR encoding,
       splitting CBOR sequences and decoding requests.
   * - ``recording.py``
     - ``RecordingWriter`` and ``read_frames`` for recording files.
   * - ``passthrough.py``, ``replay.py``
     - ``SocketPassthrough`` (TCP/UDP) and ``Replay``, which use an
       ``Outpost`` as a script of the user does.
   * - ``main.py``
     - The ``blip-outpost`` command line (``click``): ``tcp``, ``udp`` and
       ``replay``, serving until interrupted.
   * - ``timing.py``
     - ``sleep_until``, for asyncio code that samples at a steady rate.
   * - ``../examples/``
     - Not in the library: ``minimal`` (blocking code, the module functions),
       ``simulator`` (asyncio, actions as methods, also the reference) and
       ``fast`` (the stress test, encoding its messages from templates).

Threads
-------

``start`` runs a new event loop in a daemon thread, which serves until
``stop``; ``stop`` is also registered with ``atexit``, so the recording is
finished when the script just ends. ``async with`` serves from the running
event loop instead, without a thread.

``send_*`` may be called from any thread. Called in the thread of the event
loop, or while the outpost does not serve, the message goes straight to the
server's ``broadcast``. From another thread it is appended to a queue
(``collections.deque``), and a flush is scheduled in the event loop with
``call_soon_threadsafe`` -- once for all messages queued until it runs, so a
script sending in a tight loop does not wake the loop per message. The
messages of one thread keep their order.

Actions run in the event loop if they are ``async`` functions, else with
``asyncio.to_thread``, so a blocking action does not hold up the outpost.
``Actions`` guards its registry with a lock, so actions may be changed from
any thread, also from an action.

Message flow
------------

* ``send`` encodes a ``Message`` (or splits bytes of complete messages, a CBOR
  sequence, dropping what is no CBOR) and hands one message at a time to the
  server's ``broadcast``.
* ``broadcast`` drops ``ActionResult`` messages sent by the script (their ids
  are not those of any blip), keeps the latest ``Actions`` message and the
  latest ``ValueMetadata`` of each path for blips connecting later, writes the
  message to the recording, and queues it for each client. While the outpost
  does not serve, only the cache is updated.
* Each client has its own bounded queue (10 000 messages) and a sender task, so
  a slow blip neither holds up the script nor the other blips; when its queue
  is full, messages for it are dropped and counted in the log. The sender sends
  what is queued together, as a CBOR sequence in frames of up to 64 KiB, so a
  busy target does not cost a frame per message; a client connecting with
  ``?batch=0`` after the path gets one message per frame.
* ``drain`` waits until the queue of every client is at most half full; a
  script sending faster than real time calls it now and then and loses no
  messages, as long as it sends fewer than half a queue in between.
* A connecting client is checked against the path, gets the cached units and
  actions first, then everything broadcast from then on. A recording starts
  with them too.

Invocations
-----------

Each binary frame from a blip is decoded as a ``Request``. The invocation runs
in a task of its own, so a slow action does not hold up the others:

#. ``on_invoke(path, args)`` if set, else ``Actions.invoke``, which looks the
   action up, rejects it if unknown or disabled, converts and checks each
   argument against its parameter, and calls the function under the action's
   lock -- the invocations of one action run one after another, in order.
#. The answer, ``ActionResult`` with ``Ok`` or ``Err`` and the reason, goes into
   the queue of the invoking client, in order with the broadcast messages.
   Nothing about the invocation is broadcast or recorded; a rejection only
   goes to the console log of the outpost.

The passthrough sets ``on_invoke`` to forward an ``Invoke`` request with an id
of its own to the target and accept once it is sent; the replay rejects every
invocation.

Changes to ``actions`` (add, remove, enable, disable) announce the whole list
again; ``register`` adds the methods of an object in a ``batch``, announced
once.

Socket passthrough
------------------

``SocketPassthrough`` connects to the target or listens for it. Over TCP it
reads the stream in chunks, keeps the start of an incomplete message until the
rest arrives (at most 16 MiB) and forwards each complete message; data that is
no CBOR is dropped and reading starts afresh with the next chunk. Over UDP each
datagram is a CBOR sequence of complete messages.

Recording and replay
--------------------

``RecordingWriter`` writes the version 1 header, the cached units and actions,
and then each broadcast message to a gzip file, flushing every second so the
file is readable up to then also if the process is killed. ``read_frames``
reads version 1 and 2, compressed or not, several gzip members, and stops at a
cut-off message. The replay sends the frames at the pace of their timestamps
(scaled by ``speed``) and rewrites ``Actions`` messages to disabled ones.

Tests
-----

``outpost/tests`` covers the outpost as a script uses it (serving from a
thread, sending from other threads, the cache of units and actions), the
actions registry, the server (broadcast, ordering, answering, slow clients),
the socket passthrough, the recordings and the examples, with ``pytest`` and
``pytest-asyncio``::

   cd outpost && uv run pytest
