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

Putting data in
---------------

A store takes data through the ``Ingest`` trait, with typed arguments: no CBOR
and no bytes.

.. code-block:: rust

   pub trait Ingest {
       fn ingest(&mut self, source: &str, input: Input<'_>);
       fn clear(&mut self);
       fn reset(&mut self);
       fn take_notices(&mut self) -> Vec<(Level, String)> { Vec::new() }
   }

``source`` is the name of the connection or file the input came from. The
paths of traces and logs are kept as they come, without it. An ``Input`` is one
of:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Input
     - Meaning
   * - ``Sample``
     - The ``value`` of a path at a ``timestamp``, in nanoseconds.
   * - ``Samples``
     - Several values of a path (``values``, ``Rows``) taken at ``times``. A
       block counts once for the readbacks of actions.
   * - ``Latest``
     - The newest value of a path while samples are not kept (not recording):
       only the latest value of each path is kept, for the readbacks.
   * - ``Log``
     - A log record: timestamp, path, level and message.
   * - ``Metadata``
     - What describes a trace: unit, description, labels, expected range.
   * - ``Actions``
     - All actions of a source, replacing those it announced before.
   * - ``Clock``
     - The clock of the timestamps of a source, or none.

``Times`` tells when the values of a block were taken: ``Times::Period {
timestamp, period }`` (value ``i`` at ``timestamp + i * period``) or
``Times::Each(&[u64])``, a timestamp for each value. ``clear`` removes the
samples and log records and keeps what describes the sources and the latest
values; ``reset`` drops everything. ``take_notices`` is what the user is to be
told, e.g. that sources have different clocks.

You rarely call ``ingest`` yourself. A ``Session`` does it with what its sources
hand over, see :doc:`sources`, and also decides what goes in: while not
recording, a sample arrives as ``Latest`` and a log record not at all. A store
only does what it is told.

The value of a sample
---------------------

A sample holds a ``Value``, the protocol's data model (null, integer, float,
bool, text, array, record) in two buffers that are reused, so a value that is
set again allocates nothing once they grew. ``ValueRef`` borrows one; its
``node()`` says what it is, a ``Node`` that is a number, a text, ``Array`` or
``Record`` whose elements and fields are read in order. ``Rows`` is the values
of a block.

Anything that is an ``AsValue`` can be written as a value: numbers, ``bool``,
text, ``Option``, arrays and slices of these, a ``&proto::Data`` and a record
built with ``record``:

.. code-block:: rust

   use blip_kit::store::value::{record, Value};

   let mut value = Value::default();
   value.set(21.5);
   value.set(record(|r| {
       r.field("t", 21.5);
       r.field("mode", "run");
   }));

The in-memory store
-------------------

``MemoryStore`` implements ``Ingest`` and ``Store``. It keeps everything it
took in since the data was cleared, the latest value of each path, and the
actions the targets announced. It has no method that takes bytes. It ignores the
window and answers with all it has, borrowing its columns, so a plot of ten
million samples costs no copy. It is what ``Session`` holds by default. The
public fields ``traces``, ``logs`` and ``actions`` are there for the
application, e.g. to read a trace without a window. ``Series`` and ``Trace``
find a sample by time: ``index_at(t)`` (the sample current at ``t``),
``closest_index(t)`` and ``within((start, end))`` (the range of indices).

What it keeps for a recording file is not in the store: the ``Recorder`` of the
session is beside it, see :doc:`sources`. How the store ingests and keeps the
samples is in :doc:`/blip/internals`.

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

Also implement ``Ingest`` to have a session feed the store. ``Session<S>`` works
with any ``S: Ingest + Store``, so an application with a store of its own gives
it to ``Session::new(my_store)`` and the sources, the recorder and the invoker
work as with the ``MemoryStore``. The widgets read it as ``&session.store``. An
application that feeds its store itself does not need a session:
``ViewContext::new(&store, ...)`` takes any ``Store``.

Baseline
--------

``kit/tests/baseline.rs`` measures what the store and the plot cost, so that
changes to how the widgets read the store can be compared. The tests are ignored
by default:

.. code-block:: console

   cargo test --release -p blip-kit --test baseline -- --ignored --nocapture

On the development machine the store takes in about 1.8 million samples a second
while recording, 7 million while not recording, and a plot of 10 million samples
takes about 0.7 ms a frame. That was the same before and after the ``Store``
trait was introduced and before and after the entry became typed. The numbers depend on the
machine: compare runs of the same machine only.
