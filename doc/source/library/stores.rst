Stores
======

A **store** keeps what the targets sent. The widgets read it through the
``Store`` trait, not from a type, so a store may keep its data elsewhere than in
memory, e.g. in a database, and load what the widgets ask for. The trait is in
``store/query.rs``.

The Store trait
---------------

The widgets hold a ``&dyn Store``. It tells them:

* what there is: ``traces`` (every trace, with its path, kind and shape),
  ``paths_version`` (changes when a trace, a log path or a list of actions
  arrives, so widgets list the paths again only then), ``kinds``, the
  ``origin``, ``time_base`` and ``time_span`` of the data and the ``activity``
  of the timeline;
* the samples of a trace in a window: ``trace``, ``series`` (the numbers of a
  numeric trace or of an element of an array, e.g. ``imu.acc[0]``) and
  ``samples_version``, which changes when more arrives or was loaded;
* what a trace means: ``metadata``, ``unit``, ``description``, ``labels`` and
  ``expected``;
* the log records: ``logs``;
* the actions the targets announced, ``actions``, and the latest sample of the
  trace that holds the state of an action, ``readback``;
* ``pending``, whether the store is loading what a widget asked for.

Window
------

A widget asks for the samples it shows, not for all there are. A ``Window`` is
a range of time in nanoseconds and how many samples across it are of use:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Window
     - Asks for
   * - ``Window::ALL``
     - Every sample there is.
   * - ``Window::NONE``
     - No samples: what the trace is, its kind and shape, and the range of all
       its values.
   * - ``Window::between(start, end)``
     - Every sample from the start to the end.
   * - ``Window::at(t)``
     - The sample current at ``t``: the nearest before it.
   * - ``Window::of(range)``
     - Every sample of the range, all there is if ``None``.
   * - ``window.points(n)``
     - The same range with about ``n`` samples across it. A store may thin them
       out, keeping the lowest and highest of what it leaves out.

A store also gives the nearest sample before and after the range, if there is
one, so that lines and states reach its edges. It may answer with more than was
asked for.

What the widgets ask:

* The plot asks for the shown time range with two points per pixel of its
  width.
* Tooltips and tables ask ``Window::at(t)`` for the time they show.
* The traces tree and the checks of what a view can show of a trace ask
  ``Window::NONE``: they need the kind, not the samples.
* The log table asks for all records.

Loaded
------

``trace``, ``series`` and ``logs`` answer with a ``Loaded<T>``:

* ``data`` is what the store has of what was asked for.
* ``pending`` is whether the store is still loading it.

``Loaded::complete(data)`` and ``Loaded::partial(data)`` make the two. No
widget shows a loading state yet: ``pending`` is there for the application,
e.g. to show a spinner when ``Store::pending()`` is true.

The rules
---------

* The widgets ask each frame, so **a call never blocks**.
* A store that has to load what was asked for **answers with what it has**,
  sets ``pending`` and loads the rest on its own. The methods take ``&self``,
  so it notes what was asked for behind a ``Cell`` or a lock.
* When more arrived, it **asks for a repaint**, and the widget asks again.
* ``samples_version`` changes when more was loaded, so widgets that cache see
  it.

The data types
--------------

The answers borrow from the store, so the in-memory store copies nothing. A
store that loads keeps what it loaded in the same types:

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Type
     - What it is
   * - ``Trace``
     - A path with its ascending timestamps and its ``Samples``: numbers,
       strings or arrays with a shape. A record is a table of traces, one for
       each field, that share their timestamps.
   * - ``Series``
     - The numbers of a trace, or of one element of an array, as ``f64`` with
       their timestamps and the lowest and highest value of blocks of 256
       samples, which a plot reads to draw without looking at every sample.
   * - ``Traces``
     - The traces of a store by path, with their metadata.
   * - ``Logs``
     - The log records by path, those of each path in the order of their times.

The in-memory store
-------------------

``MemoryStore`` implements ``Store``. It keeps everything it received since
the data was cleared, also the received messages for saving a recording file
(``store.recording``), and the actions the targets announced. It ignores the
window and answers with all it has, borrowing its columns, so a plot of ten
million samples costs no copy. It is what ``Session`` holds. The public fields
``traces``, ``logs``, ``actions`` and ``recording`` are there for the
application, e.g. to read a trace without a window.

How it ingests and keeps the samples is in :doc:`/blip/internals`.

Writing another store
---------------------

Implement ``Store`` for your type. ``kit/tests/store.rs`` is a model: a
``Loading`` store that keeps what it loaded in a ``MemoryStore``, notes the
windows it is asked for behind a ``RefCell``, and says ``pending``. A plot reads
it, and the test checks what the plot asked for:

.. code-block:: rust

   impl Store for Loading {
       fn pending(&self) -> bool {
           true
       }
       // ...
       fn series(&self, path: &str, window: Window) -> Loaded<Option<Series<'_>>> {
           self.asked.borrow_mut().push((path.to_owned(), window));
           Loaded::partial(self.loaded.series(path, window).data)
       }
   }

The widgets work with a store that is not the session's. A ``Session`` holds a
``MemoryStore``, so an application with a store of its own reads that one in
``ViewContext::new(&store, ...)`` and feeds it itself.

Baseline
--------

``kit/tests/baseline.rs`` measures what the store and the plot cost, so that
changes to how the widgets read the store can be compared. The tests are ignored
by default:

.. code-block:: console

   cargo test --release -p blip-kit --test baseline -- --ignored --nocapture

On the development machine the store takes in about 1.5 million samples a second
and a plot of 10 million samples takes about 0.7 ms a frame. That was the same
before and after the ``Store`` trait was introduced. The numbers depend on the
machine: compare runs of the same machine only.
