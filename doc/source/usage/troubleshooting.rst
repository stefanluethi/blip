Troubleshooting
===============

The common problems, by what you see. The messages are quoted as blip and the
outpost write them; … stands for the parts that vary.

Where the messages are
----------------------

blip
   Errors show as notices in the bottom right of the views, and the reason of
   a failed connection when you hover its red dot. Problems with the data,
   e.g. a value of the wrong type, are written to blip's log: in the web app
   to the console of the browser (developer tools), in the native app to the
   terminal when started with ``RUST_LOG=warn``::

      RUST_LOG=warn target/release/blip

outpost
   Warnings and errors go to the terminal of the script or the
   ``blip-outpost`` command. ``blip-outpost`` also logs each blip that connects;
   ``--verbose`` adds debug output. In a script of your own, the info messages
   show with ``logging.basicConfig(level=logging.INFO)``.

blip does not connect
---------------------

The dot of the connection turns red, with *Cannot connect to ws://…: …*.

The outpost is not running, or listens elsewhere
   An outpost logs where it serves when it starts, e.g. *Serving blip on
   ws://127.0.0.1:3000/ws/outpost*. Make the address of the connection
   the same, see :doc:`blip/connecting`.

The path differs
   The outpost logs *Connection rejected: Path '…' does not match expected
   '…'*. Use the path of ``--ws-path`` (``/ws/outpost`` by default) in the
   address, or ``/``.

blip runs on another machine
   An outpost serves only its own machine by default (``127.0.0.1``). Start it
   with ``--ws-host 0.0.0.0``, or ``start(host="0.0.0.0")`` in a script, and
   read :doc:`limits` first. A firewall may block the port too.

The web app is served over ``https``
   The hosted web app cannot open ``ws://`` connections to other machines;
   browsers block them. Use the native app, serve the web app over ``http``,
   or put the outpost behind ``wss://``, see :doc:`installation`.

The connection breaks
---------------------

*The connection to ws://… broke: …* or *ws://… closed the connection*: the
outpost stopped, or the network went away. Connect again once it is back. While
recording, a connection that broke can be connected again and the recording goes
on; the others cannot be changed until you stop recording.

Connected, but nothing shows
----------------------------

The chip in the menu bar shows the messages received per second. If it counts
but the views stay empty:

Not recording
   blip keeps values and log records only while **Record** is on. Start it,
   see :doc:`blip/time`.

The view shows nothing
   A view shows the traces checked for it. Select the view and check traces in
   the **Traces & actions** tree, see :doc:`blip/traces`.

The window is elsewhere
   Paused, the window stays where it was. Press **Live** to follow the newest
   data.

If the chip shows no messages, the target sends nothing the outpost passes on:
see the log of the outpost.

A trace is missing or stops
---------------------------

blip's log has *…: dropped a value, … (further ones are dropped silently)*.
The first value of a path sets its kind, and the first record its fields:

* *a record cannot become …*, *a … trace cannot become a record* -- a path
  got a value of another kind than before. Keep the kind of a path the same,
  or send the other kind to a path of its own.
* *its fields are not those of the first record at the path* -- a record got
  other fields than the first one at its path. Send records of one path with
  the same fields, or ``null`` for a field without a value.
* *another trace has the path of a field*, *a field of a record has the
  path* -- a path is both a trace and a field of a record, e.g. ``imu.acc``
  sent as a value and as the field ``acc`` of the record ``imu``. Rename one.

*…: dropped a value older than the newest 65536, e.g. from a clock behind the
others sending to the path* -- two sources send to the same path, one with a
clock far behind. Send to different paths, or set the clocks alike.

Times look wrong
----------------

Times show as ``0:01:23.456`` instead of a time of day
   blip shows timestamps before 2001 as time since boot. That is right for a
   target that counts its uptime. A target that means the time of day sends
   nanoseconds since 1970: a timestamp in milliseconds or microseconds is too
   small, and shows as time since boot.

All values of a sonde tracer are at 0
   The tracer has no clock. Pass one to the tracer, see :doc:`sonde`.

Two targets do not line up
   Each target stamps its own samples. Targets that send their uptime line up
   only as far as they were started at the same time. Send the time of day
   from both, or let a script stamp the values as they arrive:
   ``send_value`` without ``timestamp_ns`` uses the time of the PC.

The outpost drops messages
--------------------------

*Client … does not keep up, dropping messages*, later *Dropped … messages for
client …*: blip took the messages slower than the outpost sent them, and the
queue of that blip (10 000 messages) was full. Show fewer views or traces, send
less often, or call ``drain()`` in the script to slow it to blip instead, see
:doc:`outpost`.

*Dropping … bytes that are no messages (…)*, *Dropping … bytes of an incomplete
message …*, *Dropping … bytes of a message that does not end …* (``tcp``,
``udp``): the target sends something that is not blip messages in CBOR, or
cuts them up. Over UDP each datagram must hold whole messages; send each batch
of sonde whole. See :doc:`reference/outpost-cli`.

protobuf targets
----------------

*Nothing from ws://… for 5 s, connecting again in 3 s*
   The target sent nothing for ``--idle-timeout`` seconds. Check that it
   sends, or raise the timeout (``0`` waits forever).
*… holds no message of kind KIND_INVOKE, so there are no actions*
   The ``--downlink`` message has no oneof of actions; see the rules in
   :doc:`reference/outpost-cli`.
*Action '…': parameter '…' has an unsupported type, the action is left out*
   Parameters are integers, floats, bools, strings and enums.

Files
-----

*… is no blip recording or layout: …*
   blip tells recordings and layouts apart by their contents; the file is
   neither, or damaged.
*… is a layout of a newer version of blip*
   The layout was saved by a newer blip. Update blip, see
   :doc:`reference/files`.
*… was opened up to message …, …*
   The recording ends in the middle of a message, e.g. as the outpost was
   killed while recording. blip shows what it holds up to there.
*Opening … was cancelled by connecting*
   Connecting while a recording is read stops reading it. Disconnect and open
   it again.

The outpost's ``replay`` warns *… is cut off*, *… is damaged* or *… ends with a
message that cannot be read: …* for the same reasons, and replays the messages
before. It refuses *… has version …, the outpost reads up to …*: a newer
recording.

Actions
-------

An action that is rejected or disabled shows the reason below it in the
actions view, if the target gives one; an invocation the target does not answer
within five seconds counts as unanswered. An action is also disabled while its
connection is not connected. See :doc:`blip/actions`.

Still stuck
-----------

Open an issue at https://github.com/stefanluethi/blip/issues with what you see,
the messages of blip and the outpost, and, if you can, a recording.
