# blip design

The look and the UI rules of blip, for anyone who designs or reviews its UI. The colour,
size and font values live in `blip/src/app/theme.rs`; this file says what they are for
and which choices were made on purpose. Studies of past choices are in this folder
(`blip-ideas.html`, `blip-actions.html`).

## Subject

blip shows what an embedded target does, live and in recordings: plots, states, tables,
XY plots, heatmaps, logs and actions. Its users are firmware developers who watch many
signals at once while they debug. The UI is a tool for reading data: the traces carry the
colour, everything else stays calm and out of the way. It should feel like Grafana or an
oscilloscope, not like a web app.

## Theme: Control Room

- Light and dark themes, both first-class. Every change is checked in both.
- The views are cards (radius `CARD_RADIUS` 4, 1 px `border`) on a darker `canvas`, with
  `GAP` 8 between and around them. Panels are bordered.
- One accent, blue (`accent`, `accent_text` on surfaces). It marks Live, the open tab,
  focus and the traces. Teal and gradients were rejected; the open tab has a solid blue
  underline.
- The selected view gets a neutral frame (`text_faint`) and a title bar filled with
  `hover`, not the accent.
- New UI takes its colours from `theme::tokens(ui.visuals())` and the helpers in
  `theme.rs`. No colour literals in views.

## Colour

- Traces use the IBM colour-blind-safe palette (`trace_color`), ordered so that
  neighbours differ most, a grey as the sixth, golden-ratio hues after that. Users do not
  pick trace colours.
- Log levels: `ok`, `warn`, `error`, `debug` as tinted pill badges (radius 4); level
  names in state bands take the same colours.
- State bands: a quiet tint (`tint`, 0.16 dark / 0.14 light) with a 2 pt top edge in the
  full colour; false bools are empty.
- Histograms use the same look for overlapping traces: a translucent wash
  (`theme::wash`, the trace colour at the `tint` opacity, so overlaps stay visible) under a
  1.5 pt outline in the full colour; all washes first, then all outlines, then markers.
  Mean and ±σ sit in a whisker strip above the bars (a row per trace: dot at the mean,
  whisker over ±σ), not as lines across the data.
- The time cursor has its own colour (`cursor_color`) and is drawn at 35 % opacity while
  it follows the end of the window.
- Keep it calm: the user found per-pixel bins and confetti-coloured log ticks in the
  timeline too noisy.

## Type

- Fira Sans Book 13 for text (Regular was too dense, Light too thin, 14 was rejected),
  with tabular digits frozen in, so changing numbers keep their place. Fira Code 12 for
  values. Small 11, heading 20. The same faces as the documentation.
- Sentence case. No all-caps labels.

## Controls and layout

- Every control is `CONTROL_HEIGHT` 24 high, so buttons, fields, checkboxes and tree rows
  line up; text fields use `FIELD_MARGIN`. Controls have radius `CONTROL_RADIUS` 3.
- Choices use `theme::segmented`, panel titles `theme::section_header`, carets
  `theme::paint_caret`, icon buttons `theme::icon_button`, tables `theme::table`. Icons
  are Phosphor.
- Left panel: the traces tree of the selected view, with a search field. Checked traces
  of plots, XY plots and histograms fill their checkbox with the trace colour; type icons sit in a
  faint right column. A check on a folder is a wildcard for new traces.
- Right panel: the properties of the selected view or group. Controls live there, not in
  the views; only the Logs view keeps its own search, filter and autoscroll.
- Bottom: the one time range of all views, with the timeline. Top: the menu bar with
  the connections chip.
- Views are added from an empty tab's picker, View › Add view or a view's ⋯ menu, and
  arranged by dragging. There is no layout panel.
- Plot legends sit in the view's title row (dot, name, value at the cursor, the shared
  unit once at the end). Stats are chips below the plot, coloured by their trace.
- Histogram legends are a dot and name per trace in the title row, no cursor value;
  without a title row, egui_plot's legend at the top right, as for plots. The axes use
  egui_plot's marks, as in plots, except the decades of a logarithmic counts axis. A
  histogram, upright or sideways, can be linked to a Plot (its only link between views):
  it takes the plot's traces, greyed in the tree, and its y range. A sideways one's
  value axis is on the left by default, like a plot, or on the right.

## Words

Say "trace" for the values of one path, "recording" for saved data. Name things by what
the user does, in plain words. Buttons say what they do; an action keeps its name through
the flow.

## Rejected on purpose

User-set trace colours, display names or hiding; a separate layout panel; egui_tiles' own
docking; teal accent; gradients; noisy, per-pixel timelines; 14 px text.
