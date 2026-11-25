Build your own app
==================

This page walks through ``kit/examples/dashboard.rs``, a whole application
built in code on ``blip-kit``. It shows two tanks that fill, simulated in the
application itself: a plot of their levels, a table with a **Drain** button
for each tank and the time controls with the timeline. There is no traces
panel, no properties panel and no layout to arrange.

.. code-block:: console

   cargo run --release -p blip-kit --example dashboard

The excerpts below are from the file. It has a test, so it keeps working, and
it builds for the web too.

What the application holds
--------------------------

.. code-block:: rust

   struct Dashboard {
       /// The time of egui in seconds, which the simulator runs by.
       clock: Rc<Cell<f64>>,
       session: Session,
       time: TimeRange,
       plot: Plot,
       table: Table,
   }

* The ``Session`` holds the store, the recorder, the invoker of actions and the
  sources.
* The ``TimeRange`` is the time all widgets show: the window, the cursor and
  whether it plays.
* ``Plot`` and ``Table`` are views, the state of a widget. They are plain
  values the application owns.

The source
----------

The example needs no outpost and no socket. Its ``Simulator`` is a ``Source``
inside the application: it tells what the target sends, and takes the
invocations of its actions. :doc:`sources` shows how it works.

.. code-block:: rust

   let mut session = Session::recording();
   session.add_source(SIMULATOR, Simulator::new(clock.clone()));

``Session::recording()`` makes a session that keeps the samples from the start,
as the application has no record button. The name given to ``add_source`` is
the name its messages are kept under and the name its actions are invoked by.
It returns a ``SourceId``, which the example does not need; keep it to remove
the source with ``remove_source(id)``.

The widgets
-----------

``view::showing`` makes a view with the traces it shows, in code:

.. code-block:: rust

   let levels = TANKS.map(|(tank, _)| format!("tank/{tank}/level"));
   // ...
   plot: view::showing(levels.clone()),
   table: view::showing::<Table>(levels).column("Drain", |ui, cell| {
       let Some(tank) = tank_of(cell.trace.path) else {
           return;
       };
       if ui.button("Drain").clicked() {
           let args = vec![("tank".to_owned(), proto::Data::from(tank))];
           cell.invoke(SIMULATOR, "drain", args);
       }
   }),

``column`` adds a column of the application to the table. Its closure draws the
cell of each row, and ``cell.invoke`` invokes an action of a source. See
:doc:`widgets`.

``theme::install(ctx)``, called once when the application starts, sets the
fonts and colours the widgets are drawn with.

The frame
---------

Each frame does the same things in the same order:

#. Update the session: it sends what was queued, receives from the sources and
   updates the invoker.
#. Update the time range from the store.
#. Show the widgets.
#. Send the invocations the widgets queued.

.. code-block:: rust

   fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
       self.update(ctx.input(|input| input.time));
       // the simulator has new samples by then, a socket would wake the app up itself
       ctx.request_repaint_after_secs(PERIOD as f32);
   }

``update`` is the first step:

.. code-block:: rust

   fn update(&mut self, now: f64) {
       self.clock.set(now);
       self.session.update(now);
       for (_, notice) in self.session.take_notices() {
           log::warn!("{notice}");
       }
   }

``Session::update(now)`` takes the time in seconds, as egui counts it. It
returns an ``Update`` with the number of messages that arrived (``messages``)
and whether the session started afresh (``started_fresh``), which the example
ignores; blip uses them for the rate and to reset what shows old data. The
notices are what went wrong in the store and with the sources, e.g. a
connection that broke. The example logs them; blip shows them as toasts, see
``Toasts`` of the workbench.

A source that wakes the application itself, as the ``WebSocket`` does, needs no
repaint timer. The simulator has no thread, so the example asks for a repaint.

The other three steps are in ``show``:

.. code-block:: rust

   fn show(&mut self, ui: &mut egui::Ui) {
       let now = ui.input(|input| input.time);
       self.time.update(&self.session.store, now);

       egui::Panel::bottom("time").show(ui, |ui| {
           // play, pause, go live and the window length, with the timeline beside them
           self.time.ui(ui, &self.session.store, true, |_| ());
       });
       egui::Panel::right("tanks")
           .default_size(320.0)
           .show(ui, |ui| {
               ui.heading("Tanks");
               let session = &mut self.session;
               let mut context =
                   ViewContext::new(&session.store, &mut self.time, &mut session.invoker);
               self.table.view(ui, &mut context);
           });
       egui::CentralPanel::default().show(ui, |ui| {
           let session = &mut self.session;
           let mut context =
               ViewContext::new(&session.store, &mut self.time, &mut session.invoker);
           self.plot.view(ui, &mut context);
       });

       // what the buttons queued goes to the simulator in this frame
       self.session.send_invocations();
   }

* ``TimeRange::update`` reads the span of the data from the store and moves the
  window on while playing.
* ``TimeRange::ui`` draws the controls and the timeline. The ``true`` is
  whether the application is recording, which the *Live* button depends on:
  going live only follows new data while recording. An application that tells
  the time range whether a source is open, with ``TimeRange::set_connected``,
  gets the hint *Press Record to collect data* on the empty timeline.
* A view is drawn with a ``ViewContext``, made from the store, the time range
  and the invoker. The views read the store and may move the time range and
  queue invocations.
* ``Session::send_invocations`` sends what the widgets queued, so a click
  reaches the source in the frame it happened in, not in the next.

The mouse wheel
---------------

The wheel zooms the plot or the timeline under the pointer. egui smooths wheel
events over several frames, so the rest of a notch would scroll whatever the
pointer moves on to. Two hooks take the events before egui does:

.. code-block:: rust

   fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
       // the wheel zooms the plot and the timeline under the pointer
       view::zoom_with_wheel(ctx, raw_input);
       time::take_wheel(ctx, raw_input);
   }

An application that leaves them out still works. The wheel then zooms a plot
only with Ctrl held, as in egui.

Web and native
--------------

The example has two ``main`` functions. Natively it runs ``eframe::run_native``.
In the browser it starts an ``eframe::WebRunner`` on the canvas with the id
``the_canvas_id`` of its page, as blip does, see ``blip/index.html`` and
``blip/src/main.rs``. The simulator reads the time of egui, not the clock of the
system, so it runs the same in both.

What to change
--------------

* To connect to an outpost instead of simulating, add a ``WebSocket`` source
  to the session, see :doc:`sources`.
* To let the user choose the traces and arrange the views, use the workbench,
  see :doc:`widgets`.
* To show your own kind of widget, write a view type, see :doc:`widgets`.
