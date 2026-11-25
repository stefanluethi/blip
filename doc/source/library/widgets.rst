Widgets
=======

The widgets are the views, the time range with its controls and timeline, and
the theme. They are in the ``widgets`` module, which needs the ``widgets``
feature and egui. They read what to show from a ``Store`` (see :doc:`stores`),
so they work with any store.

Views in code
-------------

A view is a value of a type that implements ``Viewable``. ``view::showing``
makes one with the traces it shows:

.. code-block:: rust

   use blip_kit::widgets::view::{self, plot::Plot};

   let plot: Plot = view::showing(["motor/rpm", "motor/setpoint"]);

The paths are those of traces, of the fields of a record, of actions or of log
paths, whatever the view takes from the traces tree in blip. A view that takes
nothing from the tree, e.g. one without a selection, is returned as it is.

A view draws itself with a ``ViewContext``:

.. code-block:: rust

   let mut context = ViewContext::new(&session.store, &mut time, &mut session.invoker);
   plot.view(ui, &mut context);

The context has the store as a ``&dyn Store``, the ``TimeRange`` (mutable: a
plot pans and zooms it, a click sets the cursor, as a click on a log entry does;
``LogTable::ui`` takes ``&mut TimeRange`` for that), the ``Invoker`` to queue
invocations, the display formats of the traces of the view and the plots of
the layout. ``ViewContext::new`` gives the default formats, no title row and no
plots.

The views
---------

.. list-table::
   :header-rows: 1
   :widths: 22 24 54

   * - Type
     - Module
     - Shows
   * - ``Plot``
     - ``view::plot``
     - Values over time as lines, see :doc:`/blip/views/plot`.
   * - ``States``
     - ``view::states``
     - Values over time as coloured bands, see :doc:`/blip/views/states`.
   * - ``Table``
     - ``view::table``
     - The values at the time cursor, see :doc:`/blip/views/table`.
   * - ``Xy``
     - ``view::xy``
     - Traces against other traces, see :doc:`/blip/views/xy`.
   * - ``Distribution``
     - ``view::distribution``
     - How the values spread over the shown time range, see
       :doc:`/blip/views/distribution`.
   * - ``Heatmap``
     - ``view::heatmap``
     - An array as colours, see :doc:`/blip/views/heatmap`.
   * - ``LogTable``
     - ``view::log_table``
     - The log records, see :doc:`/blip/views/logs`.
   * - ``Actions``
     - ``view::actions``
     - A form to carry out actions, see :doc:`/blip/actions`.

What each view takes and shows is in the user pages. The settings the user
changes in the properties panel are fields of the view, which an application
sets in code, e.g. ``plot.y_min``. The ends of the ranges of XY views, heatmaps
and distributions are ``Ends``: a min and a max, each ``None`` to fit the shown
values; an older preset with a fixed range or ``null`` still loads. The workbench saves them with the
layout, so the fields are serialisable, and a new field has a default so that
older presets still load.

The time range
--------------

``TimeRange`` is the time all widgets show: the window length, its end, whether
it plays, and the cursor. Each frame, call ``TimeRange::update(&store, now)``
before the widgets: it reads the span of the data from the store and moves the
window on while playing. The widgets take it from their context.

There are three ways to show its controls:

* ``TimeRange::ui(ui, &store, recording, leading)`` draws the controls with the
  timeline beside them. ``leading`` adds a widget before the controls, e.g.
  blip's record button, and its result is returned.
* ``TimeRange::controls_ui(ui, recording)`` draws the controls only: play and
  pause, going live and the window length.
* ``TimeRange::timeline_ui(ui, &store)`` draws the timeline alone, for an
  application that lays out the controls itself.

``recording`` is whether the application keeps the samples: going live only
follows new data while it does. ``TimeRange::set_connected(bool)`` tells whether
a source is open, for the hint of the empty timeline. ``toggle_play()`` does
what the play and pause button does and ``release_cursor()`` puts the cursor
back at the end of the window, e.g. for keys of the application. ``TimeRange``
is ``Clone``.

Columns of the application in a table
-------------------------------------

``Table::column(title, |ui, cell| ...)`` adds a column of the application to the
traces layout of the table, after the value. The closure draws the cell of each
row with a trace. ``TableCell`` gives it:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Field
     - What it is
   * - ``trace``
     - The trace of the row, with its samples.
   * - ``sample``
     - The index of the sample current at the shown time, if there is one.
   * - ``store``
     - The store, to read more from.
   * - ``invoker``
     - The invoker of actions.
   * - ``now``
     - The time in seconds, as egui counts it.

``cell.invoke(source, action, args)`` invokes an action, see below. Columns are
code: they are not saved with the table, so add them whenever you make the
table.

Invoking actions
----------------

An action is carried out by the source that announced it, so an invocation names
the source, as it was added to the session, and the path of the action. In a
widget of your own:

.. code-block:: rust

   let args = vec![("tank".to_owned(), proto::Data::from("a"))];
   cell.invoke("simulator", "drain", args);

``TableCell::invoke`` is the short form of ``Invoker::invoke``, which takes an
``ActionId`` (made with ``ActionId::new(source, action)``), the trace that
shows the state of the action, if any, the arguments and the time. A widget
with a ``ViewContext`` calls it through ``context.invoker``. The invoker queues
a request. ``Session::send_invocations`` sends it to the source in the
same frame, unless the source no longer announces the action. The invoker
tracks the answer: an ``ActionResult`` that arrives, or no answer after five
seconds. Actions are carried out only while recording, as in blip, see
:doc:`/blip/actions`.

Writing your own view type
--------------------------

A view type is a struct with the state of the view. It implements:

* ``Viewable``: how it is shown. ``view`` draws it in the ``ui``. Optional
  methods show its properties (``properties_ui``), give the paths it shows from
  the traces tree (``selection``), the traces it draws in the colours of the
  traces (``colored_paths``) and what it adds to its title row (``title_ui``).
* ``ViewInfo``: what it is. ``NAME`` and ``ICON`` are required, with
  ``selectable``, which tells per trace what the view can show. The constants
  ``HAS_SELECTION``, ``WRITES_VALUES``, ``SHOWS_ONE_TRACE``, ``SHOWS_ACTIONS``
  and ``SHOWS_LOGS`` have defaults.
* ``Default``, and for the workbench ``Clone``, ``serde::Serialize`` and
  ``serde::Deserialize``.

.. code-block:: rust

   use blip_kit::widgets::view::{
       Selectable, ViewContext, ViewInfo, Viewable,
   };
   use blip_kit::store::Window;
   use blip_kit::store::traces::Trace;
   use std::collections::BTreeSet;

   #[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
   struct Latest {
       #[serde(default)]
       traces: BTreeSet<String>,
   }

   impl ViewInfo for Latest {
       const NAME: &'static str = "Latest";
       const ICON: &'static str = egui_phosphor::regular::GAUGE;

       fn selectable(_trace: Trace<'_>) -> Selectable {
           Selectable::Trace
       }
   }

   impl Viewable for Latest {
       fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
           let time = context.time.cursor_timestamp();
           for path in &self.traces {
               let window = time.map_or(Window::NONE, Window::at);
               let Some(series) = context.store.series(path, window).data else {
                   continue;
               };
               if let Some(last) = series.len().checked_sub(1) {
                   ui.label(format!("{path}: {}", series.value(last)));
               }
           }
       }

       fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
           Some((&mut self.traces, false))
       }
   }

A view asks the store for what it shows, with the window it shows, see
:doc:`stores`: a view of one sample at the cursor asks ``Window::at``.

Use the view in code as any other: ``view::showing::<Latest>(["motor/rpm"])``.

Registering it for the workbench
--------------------------------

The workbench offers the view types to add in its menus. Register a type once
when the application starts, before the presets are loaded:

.. code-block:: rust

   use blip_kit::widgets::view::custom;

   custom::register::<Latest>();

``register`` needs a type that implements ``Viewable``, ``ViewInfo``,
``Default``, ``Clone``, ``Serialize`` and ``DeserializeOwned``. Registered
types are offered after the built-in ones, in the order they were registered.
``ViewType::all()`` lists both, ``ViewType::BUILT_IN`` only those of the
library. A name that is a registered one or one of the library is not
registered again, with a warning.

How a view is saved
-------------------

Built-in views are saved as before, so older presets load unchanged. A view of a
registered type is saved by its name with its state:

.. code-block:: json

   {"Custom": {"type": "Latest", "state": {"traces": ["motor/rpm"]}}}

A view whose type is not registered, e.g. in a preset made with another
application, loads as an *Unknown* placeholder, which shows no traces. It
keeps what was saved and writes it back unchanged, so saving the layout does
not lose it. Give new fields of a type a default, so older presets still load.

Wheel hooks
-----------

The plot and the timeline zoom with the wheel through two hooks of the
application, ``view::zoom_with_wheel`` and ``time::take_wheel``. Call both from
``eframe::App::raw_input_hook``, see :doc:`app`.
