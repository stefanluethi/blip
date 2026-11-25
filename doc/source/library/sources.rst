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
       /// The next event since the last call, `None` when there is none for now.
       fn poll(&mut self) -> Option<Event>;

       /// Sends `request` to the target, e.g. the invocation of an action. A source
       /// without a way back drops it.
       fn send(&mut self, request: &proto::Request);

       /// Ends the source, called before it is dropped.
       fn close(&mut self) {}
   }

The session asks each source each frame, so no call may block. That matters in
the browser, where there is nothing to wait on. A source that reads from a
thread or a socket keeps what arrived and hands it over in ``poll``.

What a source tells is an ``Event``:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Event
     - Meaning
   * - ``Opened``
     - The source is open: it delivers frames and takes requests from now on.
   * - ``Frame(bytes)``
     - A frame: one message as CBOR, or several one after another as a CBOR
       sequence (RFC 8742), e.g. a batch of sonde passed through by the
       outpost. See :doc:`/reference/protocol`.
   * - ``Closed``
     - The source ended. The session does not ask it again.
   * - ``Failed(reason)``
     - The source could not be opened or broke. The session does not ask it
       again.

``Event::message(&message)`` encodes a ``proto::Message`` as a frame, for a
source that makes its messages itself. There is no way to hand over decoded
messages without encoding them yet.

The session tells the user of a source that closed or failed as a notice, see
below.

The WebSocket source
--------------------

``WebSocket`` connects to an outpost or a target that speaks the blip protocol,
natively and in the browser. It needs the ``websocket`` feature, on by default.

.. code-block:: rust

   let socket = WebSocket::connect("ws://127.0.0.1:3000/ws/outpost", move || ctx.request_repaint())?;
   session.add_source("outpost", socket);

The second argument is called from another thread whenever an event arrived,
to wake the application up. A ``WebSocket`` is opening until it tells
``Event::Opened``.

In blip, the ``Connections`` of the workbench hold the ``WebSocket`` sources,
show their list from the status chip and pass what they receive to the
session. An application without the workbench adds its sources to the session
itself.

Writing a source
----------------

The ``Simulator`` of ``kit/examples/dashboard.rs`` is a model. It queues the
events, and ``poll`` hands them out one at a time:

.. code-block:: rust

   impl Source for Simulator {
       fn poll(&mut self) -> Option<Event> {
           if self.sent_until.is_none() {
               self.sent_until = Some(self.clock.get());
               self.describe();
               return Some(Event::Opened);
           }
           if self.queue.is_empty() {
               self.run();
           }
           self.queue.pop_front()
       }

       fn send(&mut self, request: &proto::Request) {
           let proto::Request::Invoke(invocation) = request;
           let result = match invocation.path.as_str() {
               "drain" => self.drain(&invocation.args),
               other => Err(format!("there is no action {other}")),
           };
           self.tell(proto::Message::ActionResult(proto::ActionResult {
               id: invocation.id,
               result,
           }));
       }
   }

* The first ``poll`` says what the target is: ``describe`` queues a ``Hello``,
  the ``TraceMetadata`` of each trace and the ``Actions`` it offers, then it
  returns ``Event::Opened``. What a target says once, the session keeps as the
  description of the source.
* Each later ``poll`` runs the simulation up to the time of the clock and
  queues a ``Sample`` of each tank for every period that passed. ``tell``
  encodes a message with ``Event::message`` and queues it.
* ``send`` carries out the invocation and answers with an ``ActionResult`` that
  carries the id of the invocation, which the invoker matches to the
  invocation.

The simulator reads its time from a clock the application sets each frame
(``Rc<Cell<f64>>``), not from the system, so that it runs the same natively and
in the browser. After a long pause, e.g. a hidden tab, it goes on from now
instead of catching up.

The session
-----------

``Session`` has three public fields: ``store`` (a ``MemoryStore``),
``invoker`` and ``recording``. It does what happens each frame apart from the
GUI.

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Call
     - What it does
   * - ``Session::default()``, ``Session::recording()``
     - A new session. The second keeps the samples from the start, for an
       application without a record button.
   * - ``add_source(name, source)``
     - Adds a source. Its messages are kept under ``name`` and its actions are
       invoked by it. A source of the same name is replaced.
   * - ``remove_source(name)``
     - Closes and removes the source.
   * - ``update(now)``
     - Once a frame, before the widgets: sends what is queued, receives from
       the sources and updates the invoker. ``now`` is in seconds, as egui
       counts it.
   * - ``send_invocations()``
     - After the widgets: sends what they queued.
   * - ``take_notices()``
     - What to tell the user: what went wrong in the store, and sources that
       closed or failed.
   * - ``receive_frame(source, frame, now)``, ``receive(source, message, now)``
     - Applies a frame or a message that did not come from a source of the
       session, e.g. of an opened file.
   * - ``first_opened()``
     - Tells that a source opened while no other was open. Starts afresh unless
       recording or the data is an unsaved recording. ``update`` calls it
       itself.
   * - ``set_recording(recording, connected)``
     - Starts or stops recording, see below.
   * - ``clear_data()``, ``start_fresh(store)``
     - Drops the data and the descriptions are kept, or replaces the store, e.g.
       with that of an opened recording. Both mark the data as not unsaved.
   * - ``is_unsaved()``, ``mark_saved()``
     - Whether the data holds samples recorded since it was last saved,
       cleared or replaced: ask before you drop them, e.g. to record anew or
       to quit. ``mark_saved()`` tells that you saved the data.
   * - ``waiting_for_record()``
     - Whether samples arrive but are dropped because nothing records them: a
       source is open, the session is not recording and has no samples. An
       application with a record button tells the user to press it.
   * - ``take_requests()``, ``update_invoker(connected, now)``
     - What ``send_invocations`` and ``update`` do, in parts, for an
       application that sends the requests itself, as blip does through its
       ``Connections``.

Recording
---------

Samples are kept only while recording; those that arrive otherwise are dropped.
What a target says once or seldom, its hello, its trace metadata and its
actions, is kept either way. A session that is not recording still shows the
actions and readbacks. ``Session::recording()`` records from the start.

``set_recording(true, connected)`` starts a recording only while a source is
open, and clears the data first. It returns whether one started, e.g. to follow
it live. Actions are invoked only while recording, so that the recording shows
what they did.

The first source to open while nothing is recording starts afresh: the data of
the earlier connections is dropped, unless it is an unsaved recording, which
stays until the application drops it (``start_fresh`` or ``clear_data``) after
asking the user. How a message is applied is in
:doc:`/blip/internals`.
