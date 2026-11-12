Outpost
=======

The blip-outpost library and command line.

.. req:: Serial outpost
   :id: REQ_OUTPOST_SERIAL
   :status: approved
   :component: outpost
   :satisfies: STK_DEVICE_IF

   The outpost shall forward COBS-framed CBOR messages between a serial port
   and WebSocket clients.

.. req:: Library for user scripts
   :id: REQ_OUTPOST_PROTOCOL
   :status: implemented
   :component: outpost
   :satisfies: STK_DEVICE_IF, STK_CONTROL

   The outpost shall be a Python library that a script of the user, which
   talks to the target, calls to send values, units and log records to blip
   from any thread, without blocking. The script shall declare its actions as
   Python functions whose parameters are taken from their type hints, and may
   change them at runtime.

.. req:: Sending many at once
   :id: REQ_OUTPOST_BATCH
   :status: implemented
   :component: outpost
   :satisfies: STK_DEVICE_IF, STK_LIVE_OBSERVE

   Each call of the outpost to send shall carry all of a message, its full
   path and, for a log record, its level and path, so a script
   forwards what a target sends without declaring anything first. The
   outpost shall send many samples of one trace at once, at their timestamps
   or at a fixed period, and hand all that a thread sends in a block over at
   once, with one timestamp for what is sent without one.

.. req:: Tracers for Python programs
   :id: REQ_OUTPOST_TRACERS
   :status: implemented
   :component: outpost
   :satisfies: STK_DEVICE_IF, STK_LIVE_OBSERVE

   A Python program that traces itself shall get tracers by name, as it gets
   loggers: the same name gives the same tracer anywhere in the program, its
   traces are below its name, and its log records are at its name, with
   the levels as methods. A dataclass shall be sent as a record, with the units
   its fields declare.

.. req:: Log records from Python logging
   :id: REQ_OUTPOST_LOGGING
   :status: implemented
   :component: outpost
   :satisfies: STK_DEVICE_IF, STK_LIVE_OBSERVE

   The records of Python's ``logging`` shall reach blip at the name of the
   logger as a path below a prefix, ``log`` by default, with the dots of the name
   as slashes: the logger ``app.motor`` is at ``log/app/motor``. The root
   logger shall send an empty path, or the prefix when one is given. The outpost shall append the component
   that its protobuf mapping reads from a message to the path of the record as
   one more level. The outpost shall take no component for the records it
   sends otherwise.

.. req:: Socket outpost
   :id: REQ_OUTPOST_SOCKET
   :status: implemented
   :component: outpost
   :satisfies: STK_DEVICE_IF

   The outpost shall forward messages between a TCP or UDP endpoint and
   WebSocket clients, and forward invocations to the endpoint. The endpoint
   may send several messages at once as a CBOR sequence (RFC 8742): a UDP
   datagram holds complete messages, a TCP stream may split them anywhere.
   The outpost shall forward each complete message and drop data that is no
   CBOR without stopping.

.. req:: Simulator
   :id: REQ_OUTPOST_SIM
   :status: implemented
   :component: outpost
   :satisfies: STK_DEVICE_IF, STK_CONTROL

   The outpost shall come with a simulator, an example script that streams
   synthetic CPU load and sensor values at different sampling rates, and log
   records of all severity levels, without hardware, and offers actions that
   change what it streams.

.. req:: Record messages
   :id: REQ_OUTPOST_RECORD
   :status: implemented
   :component: outpost
   :satisfies: STK_REVIEW

   The outpost shall record the messages it sends to blip to a recording file
   that blip opens.

.. req:: Replay recordings
   :id: REQ_OUTPOST_REPLAY
   :status: implemented
   :component: outpost
   :satisfies: STK_REVIEW, STK_DEVICE_IF

   The outpost shall send the messages of a recording file to blip unchanged,
   except that recorded actions are disabled, at the pace of their timestamps
   or faster.
