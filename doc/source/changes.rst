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

sonde
   * The C++17 library for firmware and unit tests: values, records, arrays and
     log records in batches, sent by the firmware or written to recording
     files by a sink.

Upgrading
   Presets replace the layout files of the development versions: blip no
   longer opens layout files, and the layout and connections it kept are
   dropped on the first start. Arrange the views again and export them as a
   preset, see :doc:`/blip/presets`.
