use crate::store::logs::{LogEntry, LogLevel, Logs, PathLogs};
use crate::store::sorted::upper_bound_by;
use crate::store::{Version, Window};
use crate::widgets::theme::{self, TokensExt};
use crate::widgets::time::TimeRange;
use crate::widgets::view::{ViewContext, Viewable};
use egui_extras::Column;
use egui_phosphor::regular;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct LogFilter {
    /// The levels whose entries are hidden.
    pub hidden_levels: BTreeSet<LogLevel>,
    /// The whole paths whose entries are hidden.
    pub hidden_paths: BTreeSet<String>,
    pub message_regex: String,
    pub case_sensitive: bool,
    pub auto_scroll: bool,
}

impl Default for LogFilter {
    fn default() -> Self {
        Self {
            hidden_levels: BTreeSet::new(),
            hidden_paths: BTreeSet::new(),
            message_regex: String::new(),
            case_sensitive: false,
            auto_scroll: true,
        }
    }
}

impl LogFilter {
    /// Whether any entries are hidden by the filters.
    fn is_active(&self) -> bool {
        !self.hidden_levels.is_empty()
            || !self.hidden_paths.is_empty()
            || !self.message_regex.is_empty()
    }

    /// Shows all entries again, but keeps whether to scroll.
    fn clear(&mut self) {
        *self = Self {
            auto_scroll: self.auto_scroll,
            ..Self::default()
        };
    }
}

/// Which columns besides the level and the message the table shows, set in the
/// properties panel.
#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(default)]
pub struct LogColumns {
    /// Whether the time shows the date too, of times in Unix time.
    pub date: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct LogTable {
    /// The paths of the shown logs, chosen in the tree of the left panel.
    pub paths: BTreeSet<String>,
    pub filter: LogFilter,
    pub columns: LogColumns,

    /// Boxed, as views are kept by value in the layout.
    #[serde(skip)]
    compiled_regex: Option<Box<Result<regex::Regex, String>>>,
    #[serde(skip)]
    last_regex_str: String,
    #[serde(skip)]
    last_case_sensitive: bool,
    /// The time range shown in the last frame, to scroll to it when it moves.
    #[serde(skip)]
    last_range: Option<(f64, f64)>,
    /// The timestamp of the cursor put somewhere in the last frame, to scroll to it
    /// when it moves.
    #[serde(skip)]
    last_cursor: Option<u64>,
    /// Whether the newest entry was followed in the last frame, to jump to it when
    /// following starts.
    #[serde(skip)]
    following: bool,
    /// Whether the user scrolled up from the newest entry, which stops following it
    /// until back at the bottom.
    #[serde(skip)]
    scrolled_away: bool,
    /// The entries the filter shows, in the order of their times, see
    /// [`LogTable::update_shown`].
    #[serde(skip)]
    shown: Vec<ShownEntry>,
    /// What `shown` was made for.
    #[serde(skip)]
    shown_for: Option<Box<ShownFor>>,
}

/// The hint of a log view with no logs checked.
const CHECK_LOGS: &str = "Check the logs to show in the tree on the left";
/// The hint of a log view whose checked logs show no entries.
const NO_ENTRIES: &str = "No entries in this time range";

/// An entry the filter shows: the index of its path in [`ShownFor::paths`] and its
/// index in the entries of that path.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ShownEntry {
    path: u32,
    index: u32,
}

/// What [`LogTable::shown`] was made for.
#[derive(Clone)]
struct ShownFor {
    /// A hash of what the filter hides.
    filter: u64,
    /// The shown paths with entries, in the order they were added in, so that the
    /// entries in [`LogTable::shown`] keep the indices of their paths.
    paths: Vec<ShownPath>,
}

/// A shown path with entries, see [`ShownFor`].
#[derive(Clone)]
struct ShownPath {
    path: Arc<str>,
    /// The order of its entries.
    order: Version,
    /// How many of its entries were filtered.
    filtered: usize,
}

/// The entries of the paths of [`ShownFor::paths`], by the indices of
/// [`ShownEntry::path`].
type Slots<'a> = Vec<&'a PathLogs>;

/// The entry `shown` of `slots`.
fn entry<'a>(slots: &[&'a PathLogs], shown: ShownEntry) -> &'a LogEntry {
    &slots[shown.path as usize].entries[shown.index as usize]
}

impl LogTable {
    fn update_regex_cache(&mut self) {
        if self.last_regex_str != self.filter.message_regex
            || self.last_case_sensitive != self.filter.case_sensitive
        {
            self.last_regex_str = self.filter.message_regex.clone();
            self.last_case_sensitive = self.filter.case_sensitive;

            if self.filter.message_regex.is_empty() {
                self.compiled_regex = None;
            } else {
                let res = regex::RegexBuilder::new(&self.filter.message_regex)
                    .case_insensitive(!self.filter.case_sensitive)
                    .build()
                    .map_err(|e| e.to_string());
                self.compiled_regex = Some(Box::new(res));
            }
        }
    }

    /// Whether the filter shows `entry`, an entry of a shown path.
    pub fn matches(&self, entry: &LogEntry) -> bool {
        if self.filter.hidden_levels.contains(&entry.level) {
            return false;
        }
        match self.compiled_regex.as_deref() {
            None => true,
            Some(Ok(re)) => re.is_match(&entry.message),
            // an invalid or incomplete pattern matches as plain text
            Some(Err(_)) if self.filter.case_sensitive => {
                entry.message.contains(&self.filter.message_regex)
            }
            Some(Err(_)) => entry
                .message
                .to_lowercase()
                .contains(&self.filter.message_regex.to_lowercase()),
        }
    }

    /// Filters the entries of the shown paths of `logs` that arrived since the last
    /// call, all of those of a path again if an entry was put before others of it, and
    /// all entries again if the filter changed or a path was no longer shown. Returns
    /// the entries of the paths with entries, by the indices of [`ShownEntry::path`].
    fn update_shown<'a>(&mut self, logs: &'a Logs) -> Slots<'a> {
        use std::hash::{Hash, Hasher};
        self.update_regex_cache();
        // without whether to scroll, which shows the same entries
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let filter = &self.filter;
        (&filter.hidden_levels, &filter.hidden_paths).hash(&mut hasher);
        (&filter.message_regex, filter.case_sensitive).hash(&mut hasher);
        let filter = hasher.finish();

        // the known paths and their entries, as long as the filter is the same and
        // they are still shown and have entries
        let known = (self.shown_for.take())
            .filter(|known| known.filter == filter)
            .and_then(|known| {
                let slots = (known.paths.iter())
                    .map(|shown| {
                        logs.get(&shown.path)
                            .filter(|_| self.paths.contains(&*shown.path))
                    })
                    .collect::<Option<Slots<'a>>>()?;
                Some((known.paths, slots))
            });
        let (mut paths, mut slots) = known.unwrap_or_else(|| {
            self.shown.clear();
            (Vec::new(), Vec::new())
        });
        // a path with an entry put before others of it is filtered again, alone
        for (slot, (known, entries)) in paths.iter_mut().zip(&slots).enumerate() {
            if known.order != entries.order() {
                self.shown.retain(|shown| shown.path != slot as u32);
                *known = ShownPath {
                    path: known.path.clone(),
                    order: entries.order(),
                    filtered: 0,
                };
            }
        }
        // the shown paths whose first entries arrived or which were picked, after the
        // known ones, all of which are shown and have entries
        let with_entries = self.paths.iter().filter(|path| logs.has_path(path));
        if with_entries.count() > paths.len() {
            let known: BTreeSet<Arc<str>> = paths.iter().map(|known| known.path.clone()).collect();
            for (path, entries) in self
                .paths
                .iter()
                .filter_map(|path| logs.get_key_value(path))
            {
                if !known.contains(path) {
                    paths.push(ShownPath {
                        path: path.clone(),
                        order: entries.order(),
                        filtered: 0,
                    });
                    slots.push(entries);
                }
            }
        }

        let mut new = Vec::new();
        for (slot, (known, entries)) in paths.iter_mut().zip(&slots).enumerate() {
            // all entries of a hidden path are hidden
            if !self.filter.hidden_paths.contains(&*known.path) {
                for (index, entry) in entries.entries.iter().enumerate().skip(known.filtered) {
                    if self.matches(entry) {
                        new.push(ShownEntry {
                            path: slot as u32,
                            index: index as u32,
                        });
                    }
                }
            }
            known.filtered = entries.entries.len();
        }
        // stable, so that entries of the same time keep the order of their paths
        let time = |shown: &ShownEntry| entry(&slots, *shown).time;
        new.sort_by_key(time);
        match (self.shown.last(), new.first()) {
            (Some(last), Some(first)) if time(last) > time(first) => {
                // an entry of one path arrived later than a newer one of another
                self.shown = merge(std::mem::take(&mut self.shown), new, time);
            }
            _ => self.shown.extend(new),
        }
        self.shown_for = Some(Box::new(ShownFor { filter, paths }));
        slots
    }

    /// The paths with entries of the last [`Self::update_shown`], by the indices of
    /// [`ShownEntry::path`].
    fn shown_paths(&self) -> &[ShownPath] {
        self.shown_for.as_ref().map_or(&[], |shown| &shown.paths)
    }

    /// Shows the entries, the ones outside the shown time range greyed out, with the
    /// filters in the header of the table. A click on an entry puts the cursor of
    /// `time` at it, and the entry at the cursor is marked.
    pub fn ui(&mut self, ui: &mut egui::Ui, logs: &Logs, time: &mut TimeRange) {
        if self.paths.is_empty() {
            theme::hint(ui, CHECK_LOGS);
            return;
        }
        let slots = self.update_shown(logs);
        let total = slots.iter().map(|path| path.entries.len()).sum();

        let footer_height = theme::CONTROL_HEIGHT;
        // the table shrinks to nothing before the footer is pushed out of a small view
        let table_height = (ui.available_height() - footer_height).max(0.0);
        // the time of the entry clicked in this frame
        let mut clicked = None;

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), table_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                let header_height = theme::table_row_height(ui, &egui::TextStyle::Body, 1);
                ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                let row_height = theme::table_row_height(ui, &egui::TextStyle::Monospace, 1);
                let lines = theme::TableLines::new(ui, true);
                // the height of the body under the header, and between its rows
                let spacing = ui.spacing().item_spacing.y;
                let body_room = ui.available_height() - header_height - spacing;

                // the paths are written relative to the folder they share
                let folder = common_folder_len(self.shown_paths().iter().map(|shown| &*shown.path));
                let columns = self.columns;
                // the time, level and path take only what they need, the
                // message the rest
                let mut builder = theme::table(ui)
                    .id_salt("log entries")
                    .min_scrolled_height(0.0)
                    // the footer stays at the bottom, also with few entries
                    .auto_shrink([false, false])
                    .sense(egui::Sense::click())
                    .column(Column::auto().resizable(false)) // Timestamp
                    .column(Column::auto().resizable(false)) // Log Level
                    .column(Column::auto().resizable(false)) // Path
                    .column(Column::remainder().at_least(150.0)); // Message

                let range = time.range();
                let timestamps = time.timestamps();
                let in_range = |entry: &LogEntry| {
                    timestamps.is_none_or(|(start, end)| (start..=end).contains(&entry.time))
                };
                // the newest entry at or before the cursor the user put somewhere
                let cursor = time.cursor_timestamp().filter(|_| time.cursor_is_set());
                let at_cursor = cursor.and_then(|cursor| {
                    upper_bound_by(&self.shown, cursor, |&shown| entry(&slots, shown).time)
                        .checked_sub(1)
                });
                let live = time.is_live();
                // live, the newest entry is followed until the user scrolls up, and
                // again once back at the bottom or with the button of the footer
                let following = self.filter.auto_scroll && live && !self.scrolled_away;
                let mut jumped = false;
                if following {
                    builder = builder.stick_to_bottom(true);
                    // whole rows only, so that no half row shows under the header
                    if let Some(body) =
                        whole_rows_height(body_room, row_height, spacing, self.shown.len())
                    {
                        builder = builder.max_scroll_height(body);
                    }
                    if !self.following && !self.shown.is_empty() {
                        // without an animation, which the user could scroll against
                        builder = builder
                            .animate_scrolling(false)
                            .scroll_to_row(self.shown.len() - 1, Some(egui::Align::Max));
                        jumped = true;
                    }
                } else if let Some(row) = at_cursor
                    // the cursor was put somewhere else, e.g. on a plot
                    .filter(|_| cursor != self.last_cursor && !time.is_playing())
                {
                    builder = builder.scroll_to_row(row, Some(egui::Align::Center));
                } else if let Some((_, end)) = timestamps.filter(|_| !live)
                    && self.filter.auto_scroll
                    // while playing, the range moves each frame
                    && (time.is_playing() || range != self.last_range)
                {
                    // scroll to the newest entry in the moved range, without an
                    // animation while playing: one that starts again with each frame
                    // never catches up with it
                    let last_in_range =
                        upper_bound_by(&self.shown, end, |&shown| entry(&slots, shown).time)
                            .saturating_sub(1);
                    builder = builder
                        .animate_scrolling(!time.is_playing())
                        .scroll_to_row(last_in_range, Some(egui::Align::Max));
                }
                self.following = following;
                self.last_range = range;
                self.last_cursor = cursor;

                let base = time.base();
                let output = builder
                    .header(header_height, |mut header| {
                        header.col(|ui| {
                            header_title(ui, "Time");
                        });
                        header.col(|ui| {
                            self.level_filter_ui(ui);
                        });
                        header.col(|ui| {
                            self.path_filter_ui(ui, folder);
                        });
                        header.col(|ui| {
                            self.message_filter_ui(ui);
                        });
                    })
                    .body(|mut body| {
                        let lines = lines.in_body(body.ui_mut());
                        let tokens = body.ui_mut().tokens();
                        let weak = body.ui_mut().visuals().weak_text_color();
                        let strong = body.ui_mut().visuals().text_color();
                        let shown_entries = &self.shown;
                        let paths = self.shown_paths();
                        body.rows(row_height, shown_entries.len(), |mut row| {
                            let index = row.index();
                            let path = &paths[shown_entries[index].path as usize].path;
                            let entry = entry(&slots, shown_entries[index]);
                            let shown = in_range(entry);
                            if at_cursor == Some(index) {
                                row.set_hovered(true);
                            }
                            // the message stands out, the time and the path less,
                            // and all of an entry outside the time range stays faint
                            let text = |text: &str, color: egui::Color32| {
                                let color = if shown { color } else { tokens.text_faint };
                                let text = egui::RichText::new(text).monospace().color(color);
                                // a click on the text clicks the row
                                egui::Label::new(text).selectable(false)
                            };
                            let time = if columns.date {
                                base.format_dated(entry.time, 3)
                            } else {
                                base.format(entry.time, 3)
                            };
                            row.col(|ui| {
                                lines.above(ui, index);
                                ui.add(text(&time, tokens.text_faint));
                            });
                            row.col(|ui| {
                                level_badge(ui, entry.level, shown);
                            });
                            row.col(|ui| {
                                let label = ui.add(text(&path[folder..], weak));
                                if folder > 0 {
                                    label.on_hover_text(&**path);
                                }
                            });
                            row.col(|ui| {
                                // cut to one line, the whole message shows on hover
                                ui.add(text(&entry.message, strong).truncate());
                            });
                            let response = row.response();
                            if response.clicked() {
                                clicked = Some(entry.time);
                            }
                            response.context_menu(|ui| {
                                if ui.button("Copy message").clicked() {
                                    ui.ctx().copy_text(entry.message.clone());
                                }
                                if ui.button("Copy row").clicked() {
                                    let level = entry.level.as_str();
                                    let message = &entry.message;
                                    let row = format!("{time}\t{level}\t{path}\t{message}");
                                    ui.ctx().copy_text(row);
                                }
                            });
                        });
                    });
                if self.shown.is_empty() {
                    theme::hint_over(ui, output.inner_rect, NO_ENTRIES);
                }

                // whether the user scrolled up from the newest entry while following
                // it, or back down
                if live && self.filter.auto_scroll {
                    if !jumped {
                        let bottom = output.content_size.y - output.inner_rect.height();
                        self.scrolled_away = output.state.offset.y + 0.5 < bottom;
                    }
                } else {
                    self.scrolled_away = false;
                }
            },
        );

        if let Some(at) = clicked {
            time.go_to(time.seconds(at));
            // the clicked entry stays where it is, also if the range moves to it
            self.last_range = time.range();
            self.last_cursor = time.cursor_timestamp();
        }
        self.footer_ui(ui, self.shown.len(), total);
    }

    /// The autoscroll button, how many entries are shown and a button to clear the
    /// filters while they hide any.
    fn footer_ui(&mut self, ui: &mut egui::Ui, shown: usize, total: usize) {
        ui.horizontal(|ui| {
            ui.set_min_height(theme::CONTROL_HEIGHT);
            let following = self.filter.auto_scroll && !self.scrolled_away;
            let auto_scroll = theme::icon_button(regular::ARROW_LINE_DOWN).selected(following);
            if ui
                .add(auto_scroll)
                .on_hover_text("Follow the newest entries")
                .clicked()
            {
                if self.scrolled_away {
                    // back to the newest entry
                    self.scrolled_away = false;
                } else {
                    self.filter.auto_scroll = !self.filter.auto_scroll;
                }
            }

            let count = if shown == total {
                format!("{total} entries")
            } else {
                format!("{shown} of {total} entries")
            };
            ui.label(egui::RichText::new(count).small().weak());

            if self.filter.is_active()
                && ui
                    .add(egui::Button::new(egui::RichText::new("Clear filters").small()).small())
                    .clicked()
            {
                self.filter.clear();
            }
        });
    }

    /// The header of the level column, with checkboxes for the shown levels.
    fn level_filter_ui(&mut self, ui: &mut egui::Ui) {
        let hidden = &mut self.filter.hidden_levels;
        filter_header(ui, "Level", !hidden.is_empty(), |ui| {
            for level in LogLevel::ALL.into_iter().rev() {
                let mut shown = !hidden.contains(&level);
                let name = egui::RichText::new(level.as_str())
                    .monospace()
                    .color(theme::level_color(ui.visuals(), level));
                if ui.checkbox(&mut shown, name).changed() {
                    super::set_member(hidden, &level, !shown);
                }
            }
            all_none_buttons(ui, |show| {
                if show {
                    hidden.clear();
                } else {
                    hidden.extend(LogLevel::ALL);
                }
            });
        });
    }

    /// The header of the path column, with checkboxes for the paths with records. The
    /// paths are written without the `folder` bytes they share.
    fn path_filter_ui(&mut self, ui: &mut egui::Ui, folder: usize) {
        let listed: BTreeSet<&str> = (self.shown_for.as_deref())
            .map_or(&[][..], |shown| &shown.paths)
            .iter()
            .map(|shown| &*shown.path)
            .collect();
        let hidden = &mut self.filter.hidden_paths;
        filter_header(ui, "Path", !hidden.is_empty(), |ui| {
            if listed.is_empty() {
                theme::panel_hint(ui, "No entries yet");
                return;
            }
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for &path in &listed {
                        let mut shown = !hidden.contains(path);
                        let checkbox = ui.checkbox(&mut shown, &path[folder..]);
                        if folder > 0 {
                            checkbox.clone().on_hover_text(path);
                        }
                        if checkbox.changed() {
                            super::set_member(hidden, path, !shown);
                        }
                    }
                });
            all_none_buttons(ui, |show| {
                if show {
                    hidden.clear();
                } else {
                    hidden.extend(listed.iter().map(|&path| path.to_owned()));
                }
            });
        });
    }

    /// The header of the message column, with the search of the messages.
    fn message_filter_ui(&mut self, ui: &mut egui::Ui) {
        let filter = &mut self.filter;
        let error = match self.compiled_regex.as_deref() {
            Some(Err(error)) => Some(error.as_str()),
            _ => None,
        };
        filter_header(ui, "Message", !filter.message_regex.is_empty(), |ui| {
            ui.horizontal(|ui| {
                let search = ui.add(
                    theme::field(&mut filter.message_regex)
                        .hint_text(format!("{} Search (regex)", regular::MAGNIFYING_GLASS))
                        .font(egui::TextStyle::Monospace)
                        .desired_width(220.0),
                );
                // type right away once the popup opens
                if !search.has_focus() && ui.memory(|memory| memory.focused().is_none()) {
                    search.request_focus();
                }

                let case = egui::Button::new(egui::RichText::new("Aa").monospace())
                    .selected(filter.case_sensitive);
                if ui.add(case).on_hover_text("Match case").clicked() {
                    filter.case_sensitive = !filter.case_sensitive;
                }

                if !filter.message_regex.is_empty()
                    && ui
                        .add(theme::icon_button(regular::X))
                        .on_hover_text("Clear the search")
                        .clicked()
                {
                    filter.message_regex.clear();
                }
            });
            if let Some(error) = error {
                let color = ui.tokens().error;
                ui.label(
                    egui::RichText::new(format!(
                        "{} Not a valid regex, searching the text",
                        regular::WARNING
                    ))
                    .small()
                    .color(color),
                )
                .on_hover_text(error);
            }
        });
    }
}

impl super::ViewInfo for LogTable {
    const NAME: &'static str = "Logs";
    const ICON: &'static str = super::trace_selection::LOG_ICON;
    const WRITES_VALUES: bool = false;
    const SHOWS_LOGS: bool = true;

    fn selectable(_trace: crate::store::traces::Trace<'_>) -> super::Selectable {
        super::Selectable::No("Log views show no traces".into())
    }
}

impl Viewable for LogTable {
    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        let records = false;
        Some((&mut self.paths, records))
    }

    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        // the table lists all records, not only those of the shown time range
        self.ui(ui, context.store.logs(Window::ALL).data, context.time);
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, _context: &super::PropertiesContext<'_>) {
        super::properties_section(ui, "Columns", |ui| {
            super::checkbox_row(
                ui,
                "Date",
                &mut self.columns.date,
                "Show the date with the time, of times in Unix time",
            );
        });
    }
}

/// `known` and `new`, each in the order of `time`, merged in that order, the known ones
/// before the new ones of the same time.
fn merge<T: Copy>(known: Vec<T>, new: Vec<T>, time: impl Fn(&T) -> u64) -> Vec<T> {
    let mut merged = Vec::with_capacity(known.len() + new.len());
    let (mut known, mut new) = (known.into_iter().peekable(), new.into_iter().peekable());
    while let (Some(a), Some(b)) = (known.peek(), new.peek()) {
        let next = if time(b) < time(a) {
            new.next()
        } else {
            known.next()
        };
        merged.extend(next);
    }
    merged.extend(known);
    merged.extend(new);
    merged
}

/// The height of the body of a table that shows whole rows of `count` rows, each
/// `row_height` high and `spacing` apart, in `room`: `None` while all rows fit, or
/// while not even one does.
fn whole_rows_height(room: f32, row_height: f32, spacing: f32, count: usize) -> Option<f32> {
    let pitch = row_height + spacing;
    let fitting = ((room + spacing) / pitch).floor();
    (fitting >= 1.0 && fitting < count as f32).then_some(fitting * pitch - spacing)
}

/// The byte length, with the trailing `/`, of the longest folder that all of `paths`
/// are in: the longest run of whole segments that is a prefix of the parents of all.
/// 0 if there is none.
fn common_folder_len<'a>(paths: impl IntoIterator<Item = &'a str>) -> usize {
    let mut folder: Option<&str> = None;
    for path in paths {
        // the parent with its trailing `/`
        let parent = path.rfind('/').map_or("", |at| &path[..=at]);
        folder = Some(match folder {
            None => parent,
            Some(common) => {
                let mut len = 0;
                for (at, _) in common.match_indices('/') {
                    if parent.starts_with(&common[..=at]) {
                        len = at + 1;
                    } else {
                        break;
                    }
                }
                &common[..len]
            }
        });
    }
    folder.map_or(0, str::len)
}

/// The title of a column without a filter, in the font of the other titles.
fn header_title(ui: &mut egui::Ui, title: &str) {
    ui.style_mut().override_text_style = None;
    ui.strong(title);
}

/// The title of a column with a filter: a button that opens `content`, the filter, in a
/// popup. The title shows in the accent color while the filter is `active`.
fn filter_header(
    ui: &mut egui::Ui,
    title: &str,
    active: bool,
    content: impl FnOnce(&mut egui::Ui),
) {
    ui.style_mut().override_text_style = None;
    let color = if active {
        ui.tokens().accent_text
    } else {
        ui.visuals().strong_text_color()
    };
    let icon = if active {
        regular::FUNNEL
    } else {
        regular::CARET_DOWN
    };
    let text = egui::RichText::new(format!("{title} {icon}"))
        .strong()
        .color(color);
    let button = theme::hover_frame(egui::Button::new(text).small());
    let response = ui.add(button);
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(content);
}

/// Buttons to show all or none of the choices of a filter, which call `show` with
/// whether to show all.
fn all_none_buttons(ui: &mut egui::Ui, mut show: impl FnMut(bool)) {
    ui.separator();
    ui.horizontal(|ui| {
        if ui.small_button("All").clicked() {
            show(true);
        }
        if ui.small_button("None").clicked() {
            show(false);
        }
    });
}

/// The level of a log record as a badge: its name in the color of the level on a tint
/// of that color, faded when the record is outside the time range.
fn level_badge(ui: &mut egui::Ui, level: LogLevel, shown: bool) {
    let fade = if shown { 1.0 } else { 0.4 };
    let color = theme::level_color(ui.visuals(), level).gamma_multiply(fade);
    let name = ui
        .painter()
        .layout_no_wrap(level.as_str().to_owned(), badge_font(ui), color);
    let (rect, response) = ui.allocate_exact_size(badge_size(ui), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, theme::CONTROL_RADIUS, color.gamma_multiply(0.15));
    painter.galley(rect.center() - name.size() / 2.0, name, color);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, level.as_str()));
}

/// The font of the names in the level badges: monospace in the small size.
fn badge_font(ui: &egui::Ui) -> egui::FontId {
    egui::FontId::monospace(egui::TextStyle::Small.resolve(ui.style()).size)
}

/// The size of the level badges: as wide as the longest name, and just higher than the
/// text.
fn badge_size(ui: &egui::Ui) -> egui::Vec2 {
    let font = badge_font(ui);
    let size = font.size;
    let widest = ui
        .painter()
        .layout_no_wrap("DEBUG".to_owned(), font, egui::Color32::PLACEHOLDER);
    egui::vec2(widest.size().x + 10.0, size + 4.0)
}

#[cfg(test)]
mod tests;
