Release notes
=============

What changed in each release of blip, the outpost and sonde, and what to do
when upgrading. The newest release is at the top. Each release lists what is
new, what changed, what was removed, and under **Upgrading** what you have to
change, e.g. in scripts, schemas, layout or recording files.

0.1.0 (not released yet)
------------------------

The first release.

blip
   * Plots, states, tables, XY plots, distributions, heatmaps, logs and actions
     views, arranged in tabs and groups.
   * Distributions of how the values spread over the shown time range, as
     histograms in percent of the samples of each trace, as box plots with
     median, mean, quartiles, whiskers and outliers, or both, upright or
     sideways; a distribution linked to a plot shows the plot's traces over
     its y range.
   * Presets: layouts with their connections, to switch between, exported to
     and imported from JSON files.
   * One time range for all views: live, paused or playing, with a time cursor
     and a timeline of all data.
   * Records with fields, arrays of one and two dimensions, units, display
     formats, and stats of the traces below plots.
   * Recording, saved compressed to ``.blip`` files and opened again.
   * Actions with buttons, checkboxes, sliders, fields and choices, carried
     out only while recording.
   * Connections to several outposts at once; in the browser and native.
   * Empty views, panels and lists show their hint in one style. Plot and States
     now say *Check the traces to show in the tree on the left* over their
     empty plot.
   * Hover tooltips of the plot, states, XY and heatmap views and of the
     distributions look alike: a framed card with a header and right-aligned
     values. A trace without a value at the pointer is now listed with a faint
     *—*. XY plots and the array line of heatmaps get the same framed tooltip.
   * Log records are told apart by their path, by convention ``log/<part>``,
     e.g. ``log/imu``, and have no component. A record without a path is at
     ``log/default``. The log table has a **Path** column, relative to the
     folder the paths share, and a **Path** filter in place of the component
     filter; checking the ``log`` folder shows all log paths below it.
   * Protocol version 1: a target says who it is in a ``Hello`` (name, clock,
     sender) and what its traces mean in a ``TraceMetadata`` (unit,
     description, labels for integer values, expected range), and may send
     blocks of samples in a ``Samples``. The connections show the target
     name, the sender and the clock; a newer protocol than blip reads gives a
     warning and an amber dot. The clock of the ``Hello`` sets the time base
     instead of the guess; when connections declare different clocks, blip
     warns once and keeps the time of day.
   * Labels: the new number format, the default for a trace with labels, writes
     *Run* for the integer 2 in States, tables, tooltips and legends. Tooltips
     show the number faint beside a label, and the description of the trace
     under the pointer. The properties section **Traces** (was **Number
     format**) and the hover of the traces show the description, the expected
     range and the labels.
   * **Fit to** *Expected* or *Values* on the Y axis of plots and the axes of XY
     views: the axis fits the range a trace declares, grown by values beyond
     it. The colour scale *Auto* of a heatmap is that range.
   * A malformed block of samples is dropped alone, with an error notice.
   * Internal: shared helpers for the timestamp searches (``model/sorted``),
     the drawing of the plot views (``view/plotting``), theme tokens, fields
     and property rows.

outpost
   * The Python library ``blip_outpost`` for scripts that connect a target:
     values, records, log records, actions, recording.
   * The ``blip-outpost`` command: pass through targets that speak the blip
     protocol over TCP or UDP, translate protobuf targets from an annotated
     schema, replay recordings.
   * Examples: a simulator, the smallest script, and a stress test.
   * ``metadata(path, unit=None, *, description, labels, value_range)`` on the
     outpost, the tracers and the module, merging per path; ``Description``,
     ``Labels`` and ``Range`` annotations on the fields of dataclass records.
   * ``Outpost(name=…, clock=…)``, also in ``start`` and ``serve``, for the
     ``Hello`` sent to each blip first, with the hostname and ``"epoch"`` by
     default. A passthrough, a replay and a protobuf link send none of their own.
   * ``series`` sends one ``Samples`` message instead of a ``Value`` for each
     sample. The simulator sends metadata and a block of vibration samples.

sonde
   * The C++17 library for firmware and unit tests: values, records, arrays and
     log records in batches, sent by the firmware or written to recording
     files by a sink. A tracer's log records are at its name.
   * Version 0.2.0: every queue sends a ``Hello`` first (``Hello{name,
     TimeBase}`` in the constructor, else the version and the sender only);
     ``metadata()`` with ``Label``, ``Range`` and ``Metadata``; ``samples()``
     with a ``Period`` or timestamps; recordings of version 3.

Upgrading
   Log records have no component any more. In sonde, ``Queue::log`` and
   ``logf`` and ``Tracer`` take no component argument; log at a path instead,
   e.g. ``log/imu``. In the outpost, ``log()``, ``get_tracer()`` and ``Tracer``
   take no ``component``; pass ``path="log/imu"``. ``LogHandler(path="")``
   sends the logger ``app.motor`` to ``log/app/motor``, below ``path`` if
   given. Old recordings still load, their component is dropped. A log view
   that showed only the old path ``log`` shows nothing until you check the
   ``log`` folder again; saved component filters of layouts are dropped.

   Presets replace the layout files of the development versions: blip no
   longer opens layout files, and the layout and connections it kept are
   dropped on the first start. Arrange the views again and export them as a
   preset, see :doc:`/blip/presets`.

   The wire messages ``Value`` and ``ValueMetadata`` are now ``Sample`` and
   ``TraceMetadata``. blip and the outpost still read the old names, in
   recordings and from targets, and the outpost forwards them unchanged. A
   script that builds messages by hand with ``Message.create_value`` and
   ``create_metadata`` needs no change. A tool of your own that reads the CBOR
   of recordings or of the WebSocket must accept the new names too.

   Recording files are version 3 now (the layout is that of before). blip and
   the outpost read versions 1 to 3; older blips and outposts refuse version 3.
   Presets files and the presets blip keeps are version 2 now, adding the
   number format ``Labels`` and ``fit``: an older blip refuses them (*holds
   presets of a newer version of blip*), a version 1 file loads without a
   notice. A trace with labels is shown with its labels unless a layout chose
   another format; choose *Default* in **Traces** to see the numbers.

   Without a ``Hello`` blip guesses the time base as before, so old targets
   need no change; add ``clock`` to the ``Hello`` of your target to make it
   certain.
