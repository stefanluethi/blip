# blip design

The look and the UI rules of blip, for anyone who designs or reviews its UI. The colour,
size and font values live in `kit/src/widgets/theme.rs`; this file says what they are for
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
- New UI takes its colours from `ui.tokens()` and the helpers in
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
- Distribution boxes use the bar look: a wash under a 1.5 pt outline, a heavier median
  line, the mean as the marker dot ringed in `card`, capped whiskers, outliers as small
  dots at 45 %. Box thickness is 40 % of the row, at most 24 pt. In Boxes, trace names
  label the rows on the category axis, with no lines or bands between rows; in Both, a
  10 pt box with a 1 pt outline sits in a 14 pt row of the marker strip.
- A choice of more than three long options is a combo box, not `segmented`.
- The time cursor has its own colour (`cursor_color`) and is drawn at 35 % opacity while
  it follows the end of the window.
- Keep it calm: the user found per-pixel bins and confetti-coloured log ticks in the
  timeline too noisy.

## Type

- Fira Sans Book 13 for text (Regular was too dense, Light too thin, 14 was rejected),
  with tabular digits frozen in, so changing numbers keep their place. Fira Code 12 for
  values. Small 11, heading 20. The same faces as the documentation.
- Sentence case. No all-caps labels. Panel and section titles too: "Traces, actions and
  logs", not "Traces & Actions".

## Controls and layout

- Every control is `CONTROL_HEIGHT` 24 high, so buttons, fields, checkboxes and tree rows
  line up; text fields use `FIELD_MARGIN`. Controls have radius `CONTROL_RADIUS` 3.
- Choices use `theme::segmented`, panel titles `theme::section_header`, carets
  `theme::paint_caret`, icon buttons `theme::icon_button`, tables `theme::table`. Icons
  are Phosphor.
- Left panel: the traces tree of the selected view, with a search field. Checked traces
  of plots, XY plots and histograms fill their checkbox with the trace colour; type icons sit in a
  faint right column. A check on a folder is a wildcard for new traces, marked by a faint
  "+" at the end of its row, and its hover tells the state. Views that show one trace
  (Heatmap) use radio buttons, not checkboxes. Without a selected view only the checkboxes
  are disabled; folders still open.
- Right panel: the properties of the selected view or group. Controls live there, not in
  the views; only the Logs view keeps its own search, filter and autoscroll.
- Bottom: the one time range of all views, with the timeline. Top: the menu bar with
  the connections chip.
- Views are added from an empty tab's picker, View › Add view or a view's ⋯ menu, and
  arranged by dragging. There is no layout panel.
- Plot legends sit in the view's title row (dot, name, value at the cursor, the shared
  unit once at the end). Stats are chips below the plot, coloured by their trace. A legend
  never hides traces: a corner legend is a picture, not a control. The title legend keeps
  whole entries while they fit, then drops the unit, then cuts a name, then ends in "…".
  Legend and tooltip show the same sample.
- One range control in every view, plot or not: "Fit to" (only while a trace declares its
  range), then Min and Max, empty to fit, with the fitted end as the faint hint. A zoomed
  axis shows its range as the hints, with a "Zoomed" row and a Fit button.
- Histogram legends are a dot and name per trace in the title row, no cursor value;
  without a title row, egui_plot's legend at the top right, as for plots. The axes use
  egui_plot's marks, as in plots, except the decades of a logarithmic counts axis. A
  histogram, upright or sideways, can be linked to a Plot (its only link between views):
  it takes the plot's traces, greyed in the tree, and its y range. A sideways one's
  value axis is on the left by default, like a plot, or on the right.
- Empty states are one short sentence that says what to do, in `text_weak` Body, with no
  icon or illustration. A view with nothing to show centres it in the card (`hint`), with
  centred lines that wrap at 320 px or the card width. When the axes or frame are still
  drawn (Plot, States, Histogram), the hint sits centred on the plot on a `card` patch so
  the grid does not cross it (`hint_over`). Panels and popups put it at the top left,
  under their header or search field (`panel_hint`). Hard line breaks are not used; the
  text wraps.
- Tooltips over the plot-like views (Plot, States, Heatmap, XY, Distribution) look alike
  and are built by one helper, modelled on the box tooltip: an egui popup frame at the
  pointer, never egui_plot's frameless label. On top a header: the time, the bin range or
  the element index in `text_weak`, or a trace's dot and name. Below it a grid of rows:
  on the left a trace's dot and name, or a label in `text_weak`; on the right the value,
  right aligned in a column as wide as its widest text, with its unit. Values that are
  missing or empty are `text_faint`. A rule may split groups of rows. The timeline's log
  list, table cells and the hints on controls keep their own look.
- What a target says about a trace (description, labels, expected range) shows where the
  trace is already named or its value written, and in the Traces section of Properties.
  No badge or icon marks a trace that has it.
- A label replaces its number; the colour follows the number. Labels is a number format,
  so the number stays one click away. An integer without a label is a plain number, not
  faint.
- A default taken from the target shows as the faint hint of the empty field it fills,
  never as a typed value; clearing the field goes back to it.
- Tooltips add the description of the one trace under the pointer only, as a footer
  below a rule, at most 3 lines.
- Status dots: green when fine, amber (`warn`) when connected with a problem, red when
  failed. Amber is only for problems.
- The Record button is inverted while connected and not recording, as the data is
  dropped: filled with the strong text colour, its label and dot in the background
  colour. Red while recording; plain and disabled while not connected.
- A field whose entry is wrong keeps it, outlined in the error colour (`view::mark`),
  with the reason on hover; the value in use is the last good one. A value that is limited
  or changed for the user is outlined in the warning colour, with what is in use on hover.
- Destructive actions ask or can be undone, never silently. What loses unsaved data asks
  "Discard the unsaved recording?" with Discard (error colour) and Cancel; Escape cancels.
  Deleting a view, tab or group, and Clear layout, take effect at once and offer Undo in a
  toast and in View › Undo delete. A connection or preset asks inline or in a dialog,
  where Enter confirms and Escape cancels.
- Toasts: info (neutral, expires), warning (expires) and error (stays until closed). A
  toast may have one action button, e.g. Undo.
- Escape goes to the innermost thing first: a field, then a menu, dialog or the connections
  list, and only then releases the time cursor. Keys such as R and Space do not fire while a
  field has the focus or a dialog is open.

## Words

Say "trace" for the values of one path, "recording" for saved data. Name things by what
the user does, in plain words. Buttons say what they do; an action keeps its name through
the flow.

## Rejected on purpose

User-set trace colours, display names or hiding; a separate layout panel; legends that
hide traces on a click; egui_tiles' own
docking; teal accent; gradients; noisy, per-pixel timelines; 14 px text.
