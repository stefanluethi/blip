Sources and sessions
====================

A **source** is where the messages of a target come from and where its requests
go to: a WebSocket to an outpost, a serial port the application reads itself, or
a simulation. A **session** receives from its sources into a store and sends
the invocations of actions back. Both are in the ``store`` module, so they work
without a GUI.

The Source trait
----------------

.. code-block:: rust

   pub trait Source {
       /// Hands what arrived since the last call to `inbox`, in order. Must not block.
       fn poll(&mut self, inbox: &mut Inbox<'_>);

       /// Sends `request` to the target, e.g. the invocation of an action. A source
       /// without a way back drops it.
       fn send(&mut self, request: &proto::Request);

       /// Ends the source, called before it is dropped.
       fn close(&mut self) {}
   }

The session asks each source each frame, so no call may block. That matters in
the browser, where there is nothing to wait on. A source that reads from a
thread or a socket keeps what arrived and hands it over in ``poll``.

A source hands what it has to the ``Inbox`` it gets in ``poll``. The session
makes the inbox for one source and one poll, and does the rest: it keeps the
data while recording and only the latest value of each path otherwise, answers
invocations and records what arrives. A source does not know whether the
session records.

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Call of the inbox
     - Meaning
   * - ``opened()``
     - The source is open: it delivers messages and takes requests from now
       on. Call it before the first message.
   * - ``closed()``, ``failed(reason)``
     - The source ended, or could not be opened or broke. The session drops it
       and tells the user. The inbox ignores every call after these.
   * - ``frame(bytes)``
     - Bytes in the blip protocol: one message as CBOR, or several one after
       another as a CBOR sequence (RFC 8742), e.g. a batch of sonde passed
       through by the outpost. See :doc:`/reference/protocol`. The library
       reads them, so a source holds no parser. A message that cannot be read
       is logged and not shown, and a malformed block of samples gives a
       notice.
   * - ``sample(timestamp, path, value)``
     - The value of a trace at a time, in nanoseconds. ``value`` is anything
       that is an ``AsValue``: a number, a bool, text, an array or a record,
       see :doc:`stores`.
   * - ``samples(path, times, values)``
     - A block of values taken at ``times``, a ``Times`` with a period or a
       timestamp for each value. A block ends with its timestamps for
       ``Times::Each`` and with its values for ``Times::Period``.
   * - ``log(timestamp, path, level, message)``
     - A log record. Logs are kept only while recording.
   * - ``metadata(&metadata)``, ``actions(&actions)``, ``hello(&hello)``
     - What the target says about itself: its traces, its actions and who it
       is.
   * - ``action_result(&result)``
     - The answer of the target to an invocation.
   * - ``message(&message)``
     - A ``proto::Message`` the source built itself.

The session tells the user of a source that closed or failed as a notice, see
below.

The WebSocket source
--------------------

``WebSocket`` connects to an outpost or a target that speaks the blip protocol,
natively and in the browser. It needs the ``websocket`` feature, on by default.

.. code-block:: rust

   let socket = WebSocket::connect("ws://127.0.0.1:3000/ws/outpost", move || ctx.request_repaint())?;
   let id = session.add_source("outpost", socket);

The second argument is called from another thread whenever an event arrived,
to wake the application up. A ``WebSocket`` is opening until it calls
``inbox.opened()``. In ``poll`` it hands each binary message to
``inbox.frame``, as received; it splits and reads nothing.

In blip, the ``Connections`` of the workbench show the list of addresses from
the status chip and add a ``WebSocket`` to the session for each open one, with
the address as its label. After each ``Session::update`` they read what
happened with ``Connections::observe(&session)``, from the ``source_events`` of
the session. An application without the workbench adds its sources to the
session itself.

Writing a source
----------------

There are two kinds, by what the source has.

**Bytes in the blip protocol**, e.g. a serial port that the application reads and
deframes itself, hand them over as they are:

.. code-block:: rust

   struct Serial {
       port: Port,
       opened: bool,
   }

   impl Source for Serial {
       fn poll(&mut self, inbox: &mut Inbox<'_>) {
           if !self.opened {
               self.opened = true;
               inbox.opened();
           }
           while let Some(frame) = self.port.read_frame() {
               inbox.frame(&frame);
           }
       }

       fn send(&mut self, request: &proto::Request) {
           // a source without a way back leaves this empty
       }
   }

**Data of its own**, e.g. a simulation or a sensor the application reads, hand
over typed, without encoding it and without building a protocol message:

.. code-block:: rust

   use blip_kit::store::Times;
   use blip_kit::store::session::Session;

   let mut session = Session::recording();
   let mut inbox = session.inbox("sim", 0.0);
   inbox.opened();
   inbox.sample(1_000, "motor.speed", 3.2);
   inbox.samples("motor.current", Times::Period { timestamp: 2_000, period: 1_000 }, [0.5, 0.6]);
   inbox.log(3_000, "motor", 2, "started");

``Session::inbox(source, now)`` gives an inbox without a ``Source`` to poll, for
an application that has its data at hand. A ``Source`` gets its inbox in
``poll``.

The ``Simulator`` of ``kit/examples/dashboard.rs`` is a model of the second
kind. Its ``poll`` runs the simulation and hands over what happened:

.. code-block:: rust

   impl Source for Simulator {
       fn poll(&mut self, inbox: &mut Inbox<'_>) {
           if self.sent_until.is_none() {
               self.sent_until = Some(self.clock.get());
               inbox.opened();
               self.describe(inbox);
           }
           self.run(inbox);
           for answer in self.answers.drain(..) {
               match answer {
                   Answer::Log {
                       timestamp,
                       level,
                       text,
                   } => inbox.log(timestamp, "log/tanks", level, &text),
                   Answer::Result(result) => inbox.action_result(&result),
               }
           }
       }

       fn send(&mut self, request: &proto::Request) {
           let proto::Request::Invoke(invocation) = request;
           let result = match invocation.path.as_str() {
               "drain" => self.drain(&invocation.args),
               other => Err(format!("there is no action {other}")),
           };
           self.answers.push(Answer::Result(proto::ActionResult {
               id: invocation.id,
               result,
           }));
       }
   }

* The first ``poll`` opens the source, then ``describe`` says what the target
  is: ``inbox.hello``, ``inbox.metadata`` for each trace and ``inbox.actions``.
  What a target says once, the session keeps as the description of the source.
* Each ``poll`` runs the simulation up to the time of the clock and hands over
  ``inbox.sample(timestamp, path, level)`` of each tank for every period that
  passed, and ``Answer::Log`` when a tank is full. ``Answer`` is a private enum of the
  example: ``Log { timestamp, level, text }`` or ``Result(proto::ActionResult)``.
* ``send`` carries out the invocation and queues an ``Answer::Result`` with an ``ActionResult`` that
  carries the id of the invocation, which the invoker matches to the invocation.
  The next ``poll`` hands it over with ``inbox.action_result``, and the logs
  with ``inbox.log``. The example builds no protocol message.

The simulator reads its time from a clock the application sets each frame
(``Rc<Cell<f64>>``), not from the system, so that it runs the same natively and
in the browser. After a long pause, e.g. a hidden tab, it goes on from now
instead of catching up.

The session
-----------

``Session<S>`` works with any store ``S`` that takes input and answers queries
(``Ingest + Store``, see :doc:`stores`); the default is the ``MemoryStore``. It
has four public fields: ``store``, ``recorder`` (what writes the recording file
from what arrives), ``invoker`` and ``recording`` (whether samples are kept). It
does what happens each frame apart from the GUI.

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Call
     - What it does
   * - ``Session::default()``, ``Session::recording()``, ``Session::new(store)``
     - A new session over a ``MemoryStore``, or over the store of the
       application. ``recording()`` keeps the samples from the start, for an
       application without a record button.
   * - ``add_source(name, source)``
     - Adds a source and returns its ``SourceId``. Its messages are kept under
       ``name`` and its actions are invoked by it. Sources may share a name,
       e.g. two connections without one; nothing is replaced, the id tells
       them apart.
   * - ``remove_source(id)``
     - Closes and removes the source. It tells nothing: no event, no notice.
   * - ``set_label(id, label)``
     - What the notices call the source instead of its name, e.g. the address
       of a connection: *The connection to ws://host:3000 broke*.
   * - ``update(now)``
     - Once a frame, before the widgets: sends what is queued, receives from
       the sources and updates the invoker. ``now`` is in seconds, as egui
       counts it. Returns an ``Update``: ``messages``, how many arrived, e.g.
       for a rate, and ``started_fresh``, whether a source opened while none
       was open and the session dropped the data (unless recording).
   * - ``source_events()``
     - What the sources told in the last ``update``, in order, each with its
       ``SourceId``: opened, ``Hello``, closed, failed. A source that closed or
       failed is no longer in the session.
   * - ``send_invocations()``
     - After the widgets: sends what they queued.
   * - ``take_notices()``
     - What to tell the user: what went wrong in the store, messages that could
       not be read and sources that closed or failed.
   * - ``inbox(source, now)``
     - An inbox to hand over messages without a source to poll, see above.
   * - ``first_opened()``
     - Tells that a source opened while no other was open. Starts afresh unless
       recording. ``update`` calls it itself.
   * - ``set_recording(recording, connected)``
     - Starts or stops recording, see below.
   * - ``clear_data()``, ``start_fresh(store, recorder)``
     - Drops the data and keeps the descriptions, or replaces the store and the
       recorder, e.g. with those of an opened recording.
   * - ``take_requests()``, ``update_invoker(connected, now)``
     - What ``send_invocations`` and ``update`` do, in parts, for an
       application that sends the requests itself.

An invocation names the source it is for, not its id. When several open sources
share that name, the request goes to the one that announced its actions last,
the actions the store holds and the user sees; if none did, to the first open
one.

Recording
---------

Samples are kept only while recording; of those that arrive otherwise, only the
latest value of each path is kept, for the readbacks of actions. What a target
says once or seldom, its hello, its trace metadata and its actions, is kept
either way. A session that is not recording still shows the actions and
readbacks. ``Session::recording()`` records from the start.

The ``recorder`` of the session encodes what arrives, as it arrives, for the
recording file: ``session.recorder.to_file()`` is the file, and
``session.recorder.messages()`` how many messages it holds. It is beside the
store, not in it, so a store of the application needs nothing for it. The file
holds the same as was received, not the same bytes: see
:doc:`/reference/protocol`. A message the library cannot read is kept in it as it
arrived.

``set_recording(true, connected)`` starts a recording only while a source is
open, and clears the data first. It returns whether one started, e.g. to follow
it live. Actions are invoked only while recording, so that the recording shows
what they did.

The first source to open while nothing is recording starts afresh: the data of
the earlier connections is dropped. How a message is applied is in
:doc:`/blip/internals`.
