use crate::app::model::Version;
use crate::app::model::logs::{LogEntry, LogLevel, Logs, PathLogs};
use crate::app::theme;
use crate::app::time::{TimeBase, TimeRange};
use crate::app::view::{ViewContext, Viewable};
use egui_extras::Column;
use egui_phosphor::regular;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct LogFilter {
    /// The levels whose entries are hidden.
    pub hidden_levels: BTreeSet<LogLevel>,
    /// The components whose entries are hidden, so that new components show.
    pub hidden_components: BTreeSet<String>,
    pub message_regex: String,
    pub case_sensitive: bool,
    pub auto_scroll: bool,
}

impl Default for LogFilter {
    fn default() -> Self {
        Self {
            hidden_levels: BTreeSet::new(),
            hidden_components: BTreeSet::new(),
            message_regex: String::new(),
            case_sensitive: false,
            auto_scroll: true,
        }
    }
}

impl LogFilter {
    /// Whether any entries are hidden by the filters, of the components only if
    /// `components` are shown.
    fn is_active(&self, components: bool) -> bool {
        !self.hidden_levels.is_empty()
            || components && !self.hidden_components.is_empty()
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
#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct LogColumns {
    /// Whether the time shows the date too, of times in Unix time.
    pub date: bool,
    /// Whether the component and its filter show. Its filter hides nothing while not.
    pub component: bool,
}

impl Default for LogColumns {
    fn default() -> Self {
        Self {
            date: false,
            component: true,
        }
    }
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
    /// The entries the filter shows, in the order of their times, see
    /// [`LogTable::update_shown`].
    #[serde(skip)]
    shown: Vec<ShownEntry>,
    /// What `shown` was made for.
    #[serde(skip)]
    shown_for: Option<Box<ShownFor>>,
}

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
    /// How many of them the filter shows.
    shown: usize,
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
        if self.filter.hidden_levels.contains(&entry.level)
            || self.columns.component && self.filter.hidden_components.contains(&entry.component)
        {
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
        self.columns.component.hash(&mut hasher);
        (&filter.hidden_levels, &filter.hidden_components).hash(&mut hasher);
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
                    shown: 0,
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
                        shown: 0,
                    });
                    slots.push(entries);
                }
            }
        }

        let mut new = Vec::new();
        for (slot, (known, entries)) in paths.iter_mut().zip(&slots).enumerate() {
            for (index, entry) in entries.entries.iter().enumerate().skip(known.filtered) {
                if self.matches(entry) {
                    new.push(ShownEntry {
                        path: slot as u32,
                        index: index as u32,
                    });
                    known.shown += 1;
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

    /// Whether the shown entries are of several paths, so that the path tells them apart.
    fn shows_several_paths(&self) -> bool {
        (self.shown_paths().iter())
            .filter(|path| path.shown > 0)
            .count()
            > 1
    }

    /// Shows the entries, the ones outside the shown time range greyed out, with the
    /// filters in the header of the table.
    pub fn ui(&mut self, ui: &mut egui::Ui, logs: &Logs, time: &TimeRange) {
        let slots = self.update_shown(logs);
        let total = slots.iter().map(|path| path.entries.len()).sum();

        let footer_height = 20.0;
        // the table shrinks to nothing before the footer is pushed out of a small view
        let table_height = (ui.available_height() - footer_height).max(0.0);

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), table_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                let header_height = theme::table_row_height(ui, &egui::TextStyle::Body, 1);
                ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                let row_height = theme::table_row_height(ui, &egui::TextStyle::Monospace, 1);
                let lines = theme::TableLines::new(ui, true);

                // the path tells the entries apart only if they are of several paths
                let with_path = self.shows_several_paths();
                let columns = self.columns;
                // the time, level, path and component take only what they need, the
                // message the rest
                let mut builder = theme::table(ui)
                    .id_salt("log entries")
                    .min_scrolled_height(0.0)
                    // the footer stays at the bottom, also with few entries
                    .auto_shrink([false, false])
                    .column(Column::auto().resizable(false)) // Timestamp
                    .column(Column::auto().resizable(false)); // Log Level
                if with_path {
                    builder = builder.column(Column::auto().resizable(false)); // Path
                }
                if columns.component {
                    builder = builder.column(Column::auto().resizable(false)); // Component
                }
                builder = builder.column(Column::remainder().at_least(150.0)); // Message

                let range = time.range();
                let timestamps =
                    range.map(|(start, end)| (time.timestamp(start), time.timestamp(end)));
                let in_range = |entry: &LogEntry| {
                    timestamps.is_none_or(|(start, end)| (start..=end).contains(&entry.time))
                };
                // the newest entry, also of the range while playing, is followed
                // without an animation: one that starts again with each new entry
                // never catches up with it
                let following = self.filter.auto_scroll && time.is_playing();
                if time.is_live() {
                    if following && !self.shown.is_empty() {
                        builder = builder
                            .animate_scrolling(false)
                            .scroll_to_row(self.shown.len() - 1, Some(egui::Align::Max));
                    }
                } else if let Some((_, end)) = timestamps
                    // while playing, the range moves each frame
                    && (following || !time.is_playing() && range != self.last_range)
                {
                    // scroll to the newest entry in the moved range
                    let last_in_range = self
                        .shown
                        .partition_point(|&shown| entry(&slots, shown).time <= end)
                        .saturating_sub(1);
                    builder = builder
                        .animate_scrolling(!following)
                        .scroll_to_row(last_in_range, Some(egui::Align::Max));
                }
                self.last_range = range;

                builder
                    .header(header_height, |mut header| {
                        header.col(|ui| {
                            header_title(ui, "Time");
                        });
                        header.col(|ui| {
                            self.level_filter_ui(ui);
                        });
                        if with_path {
                            header.col(|ui| {
                                header_title(ui, "Path");
                            });
                        }
                        if columns.component {
                            header.col(|ui| {
                                self.component_filter_ui(ui, &slots);
                            });
                        }
                        header.col(|ui| {
                            self.message_filter_ui(ui);
                        });
                    })
                    .body(|mut body| {
                        let lines = lines.in_body(body.ui_mut());
                        let shown_entries = &self.shown;
                        let paths = self.shown_paths();
                        body.rows(row_height, shown_entries.len(), |mut row| {
                            let index = row.index();
                            let path = &paths[shown_entries[index].path as usize].path;
                            let entry = entry(&slots, shown_entries[index]);
                            let shown = in_range(entry);
                            // the message stands out, the time and the component less,
                            // and all of an entry outside the time range stays faint
                            let text = |text: &str, ui: &egui::Ui, role: Role| {
                                let tokens = theme::tokens(ui.visuals());
                                let color = match role {
                                    _ if !shown => tokens.text_faint,
                                    Role::Time => tokens.text_faint,
                                    Role::Path | Role::Component => ui.visuals().weak_text_color(),
                                    Role::Message => ui.visuals().text_color(),
                                };
                                egui::RichText::new(text).monospace().color(color)
                            };
                            row.col(|ui| {
                                lines.above(ui, index);
                                let base = TimeBase::guess(entry.time);
                                let time = if columns.date {
                                    base.format_dated(entry.time, 3)
                                } else {
                                    base.format(entry.time, 3)
                                };
                                ui.label(text(&time, ui, Role::Time));
                            });
                            row.col(|ui| {
                                level_badge(ui, entry.level, shown);
                            });
                            if with_path {
                                row.col(|ui| {
                                    ui.label(text(path, ui, Role::Path));
                                });
                            }
                            if columns.component {
                                row.col(|ui| {
                                    ui.label(text(&entry.component, ui, Role::Component));
                                });
                            }
                            row.col(|ui| {
                                ui.label(text(&entry.message, ui, Role::Message));
                            });
                        });
                    });
            },
        );

        self.footer_ui(ui, self.shown.len(), total);
    }

    /// The autoscroll button, how many entries are shown and a button to clear the
    /// filters while they hide any.
    fn footer_ui(&mut self, ui: &mut egui::Ui, shown: usize, total: usize) {
        ui.horizontal(|ui| {
            // smaller than the other buttons, to leave the room to the entries
            let icon = egui::RichText::new(regular::ARROW_LINE_DOWN).size(12.0);
            let auto_scroll = theme::hover_frame(
                egui::Button::new(icon)
                    .small()
                    .min_size(egui::vec2(18.0, 18.0))
                    .selected(self.filter.auto_scroll),
            );
            if ui
                .add(auto_scroll)
                .on_hover_text("Follow the newest entries")
                .clicked()
            {
                self.filter.auto_scroll = !self.filter.auto_scroll;
            }

            let count = if shown == total {
                format!("{total} entries")
            } else {
                format!("{shown} of {total} entries")
            };
            ui.label(egui::RichText::new(count).small().weak());

            if self.filter.is_active(self.columns.component)
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
                    toggle(hidden, level, shown);
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

    /// The header of the component column, with checkboxes for the components of the
    /// shown paths, `slots`.
    fn component_filter_ui(&mut self, ui: &mut egui::Ui, slots: &[&PathLogs]) {
        let hidden = &mut self.filter.hidden_components;
        filter_header(ui, "Component", !hidden.is_empty(), |ui| {
            let components: BTreeSet<&str> = (slots.iter())
                .flat_map(|path| &path.entries)
                .map(|entry| entry.component.as_str())
                .collect();
            if components.is_empty() {
                ui.weak("No components yet");
                return;
            }
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for &component in &components {
                        let mut shown = !hidden.contains(component);
                        if ui.checkbox(&mut shown, component).changed() {
                            toggle(hidden, component.to_owned(), shown);
                        }
                    }
                });
            all_none_buttons(ui, |show| {
                if show {
                    hidden.clear();
                } else {
                    hidden.extend(components.iter().map(|&c| c.to_owned()));
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
                    egui::TextEdit::singleline(&mut filter.message_regex)
                        .margin(theme::FIELD_MARGIN)
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
                let color = theme::tokens(ui.visuals()).error;
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

    fn selectable(_trace: crate::app::model::traces::Trace<'_>) -> super::Selectable {
        super::Selectable::No("Log views show no traces".into())
    }
}

impl Viewable for LogTable {
    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        let records = false;
        Some((&mut self.paths, records))
    }

    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        self.ui(ui, &context.model.logs, context.time);
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, _context: &super::PropertiesContext<'_>) {
        super::properties_section(ui, "Columns", |ui| {
            super::property_label(ui, "Date");
            ui.checkbox(&mut self.columns.date, "")
                .on_hover_text("Show the date with the time, of times in Unix time");
            ui.end_row();
            super::property_label(ui, "Component");
            ui.checkbox(&mut self.columns.component, "")
                .on_hover_text("Show the component of the entries, to filter by it");
            ui.end_row();
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
        theme::tokens(ui.visuals()).accent_text
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

/// Removes `item` from the `hidden` ones if `shown`, else adds it.
fn toggle<T: Ord>(hidden: &mut BTreeSet<T>, item: T, shown: bool) {
    if shown {
        hidden.remove(&item);
    } else {
        hidden.insert(item);
    }
}

/// The level of a log record as a badge: its name in the color of the level on a tint
/// of that color, faded when the record is outside the time range.
fn level_badge(ui: &mut egui::Ui, level: LogLevel, shown: bool) {
    let fade = if shown { 1.0 } else { 0.4 };
    let color = theme::level_color(ui.visuals(), level).gamma_multiply(fade);
    let name = ui
        .painter()
        .layout_no_wrap(level.as_str().to_owned(), BADGE_FONT, color);
    let (rect, response) = ui.allocate_exact_size(badge_size(ui), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, theme::CONTROL_RADIUS, color.gamma_multiply(0.15));
    painter.galley(rect.center() - name.size() / 2.0, name, color);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, level.as_str()));
}

/// The font of the names in the level badges.
const BADGE_FONT: egui::FontId = egui::FontId::monospace(10.5);

/// The size of the level badges: as wide as the longest name, and just higher than the
/// text.
fn badge_size(ui: &egui::Ui) -> egui::Vec2 {
    let widest =
        ui.painter()
            .layout_no_wrap("DEBUG".to_owned(), BADGE_FONT, egui::Color32::PLACEHOLDER);
    egui::vec2(widest.size().x + 10.0, BADGE_FONT.size + 4.0)
}

/// What a text of a log entry is, which sets its color.
#[derive(Clone, Copy)]
enum Role {
    Time,
    Path,
    Component,
    Message,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::logs::DEFAULT_PATH;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// A table showing the logs without a path.
    fn table() -> LogTable {
        LogTable {
            paths: BTreeSet::from([DEFAULT_PATH.to_owned()]),
            ..LogTable::default()
        }
    }

    fn create_entry(level: LogLevel, component: &str, message: &str) -> LogEntry {
        LogEntry {
            time: 0,
            component: component.to_string(),
            level,
            message: message.to_string(),
        }
    }

    /// The paths and the indices of the shown entries.
    fn shown(table: &LogTable) -> Vec<(&str, u32)> {
        let paths = &table.shown_for.as_ref().unwrap().paths;
        (table.shown.iter())
            .map(|shown| (&*paths[shown.path as usize].path, shown.index))
            .collect()
    }

    /// The indices of the shown entries of the logs without a path.
    fn shown_indices(table: &LogTable) -> Vec<u32> {
        shown(table).into_iter().map(|(_, index)| index).collect()
    }

    #[test]
    fn new_entries_are_filtered_and_all_again_when_the_filter_or_order_changes() {
        let at = |time, level| LogEntry {
            time,
            ..create_entry(level, "core", "message")
        };
        let mut logs = Logs::default();
        logs.push("", at(1, LogLevel::Info));
        logs.push("", at(2, LogLevel::Warn));
        let mut table = table();
        table.filter.hidden_levels.insert(LogLevel::Info);
        table.update_shown(&logs);
        assert_eq!(shown_indices(&table), [1]);

        logs.push("", at(3, LogLevel::Warn));
        logs.push("", at(4, LogLevel::Info));
        table.update_shown(&logs);
        assert_eq!(shown_indices(&table), [1, 2]);

        table.filter.hidden_levels.clear();
        table.update_shown(&logs);
        assert_eq!(shown_indices(&table), [0, 1, 2, 3]);

        // an entry before the others moves them on
        table.filter.hidden_levels.insert(LogLevel::Info);
        table.update_shown(&logs);
        logs.push("", at(0, LogLevel::Warn));
        table.update_shown(&logs);
        assert_eq!(shown_indices(&table), [0, 2, 3]);
    }

    /// Records at "motor" and at "robot/arm".
    fn logs_of_two_paths() -> Logs {
        let mut logs = Logs::default();
        for path in ["motor", "robot/arm"] {
            logs.push(path, create_entry(LogLevel::Info, "core", "message"));
        }
        logs
    }

    #[test]
    fn only_the_logs_of_the_picked_paths_are_shown() {
        let logs = logs_of_two_paths();
        let mut table = LogTable {
            paths: BTreeSet::from(["motor".to_owned()]),
            ..LogTable::default()
        };
        table.update_shown(&logs);
        assert_eq!(shown(&table), [("motor", 0)]);
        table.paths.insert("robot/arm".to_owned());
        table.update_shown(&logs);
        assert_eq!(shown(&table), [("motor", 0), ("robot/arm", 0)]);
        // a path no longer picked is no longer shown
        table.paths.remove("motor");
        table.update_shown(&logs);
        assert_eq!(shown(&table), [("robot/arm", 0)]);
    }

    #[test]
    fn the_entries_of_several_paths_are_shown_in_the_order_of_their_times() {
        let at = |time| LogEntry {
            time,
            ..create_entry(LogLevel::Info, "core", "message")
        };
        let mut logs = Logs::default();
        logs.push("motor", at(1));
        logs.push("robot", at(2));
        logs.push("motor", at(3));
        let mut table = LogTable {
            paths: BTreeSet::from(["motor".to_owned(), "robot".to_owned()]),
            ..LogTable::default()
        };
        table.update_shown(&logs);
        assert_eq!(shown(&table), [("motor", 0), ("robot", 0), ("motor", 1)]);
        // appended to its path, but before an entry of the other one
        logs.push("robot", at(2));
        logs.push("motor", at(4));
        table.update_shown(&logs);
        let expected = [
            ("motor", 0),
            ("robot", 0),
            ("robot", 1),
            ("motor", 1),
            ("motor", 2),
        ];
        assert_eq!(shown(&table), expected);
        // a path whose first entry arrives, after the known ones of the same time
        table.paths.insert("arm".to_owned());
        logs.push("arm", at(3));
        table.update_shown(&logs);
        assert_eq!(shown(&table)[3..5], [("motor", 1), ("arm", 0)]);
        // an entry before the others of its path, which alone is filtered again
        logs.push("robot", at(0));
        table.update_shown(&logs);
        let expected = [
            ("robot", 0),
            ("motor", 0),
            ("robot", 1),
            ("robot", 2),
            ("motor", 1),
            ("arm", 0),
            ("motor", 2),
        ];
        assert_eq!(shown(&table), expected);
        let counts: Vec<_> = (table.shown_paths().iter())
            .map(|path| (&*path.path, path.shown))
            .collect();
        assert_eq!(counts, [("motor", 3), ("robot", 3), ("arm", 1)]);
    }

    #[test]
    fn the_path_is_shown_while_the_logs_of_several_paths_are() {
        let harness = |paths: &[&str]| {
            let logs = logs_of_two_paths();
            let mut table = LogTable {
                paths: paths.iter().map(|path| path.to_string()).collect(),
                ..LogTable::default()
            };
            let time = TimeRange::default();
            Harness::builder()
                .with_size(egui::vec2(600.0, 300.0))
                .build_ui(move |ui| table.ui(ui, &logs, &time))
        };
        let several = harness(&["motor", "robot/arm"]);
        several.get_by_label("Path");
        several.get_by_label("robot/arm");
        assert!(harness(&["motor"]).query_by_label("Path").is_none());
        // paths picked without shown entries tell nothing apart
        assert!(harness(&["motor", "log"]).query_by_label("Path").is_none());
    }

    #[test]
    fn only_the_paths_of_the_shown_entries_count() {
        let mut logs = Logs::default();
        logs.push("", create_entry(LogLevel::Info, "core", "message"));
        logs.push("motor", create_entry(LogLevel::Info, "hidden", "message"));
        let mut table = LogTable {
            paths: BTreeSet::from([DEFAULT_PATH.to_owned(), "motor".to_owned()]),
            ..LogTable::default()
        };
        // the entry of "motor" is hidden
        table.filter.hidden_components.insert("hidden".to_owned());
        table.update_shown(&logs);
        assert!(!table.shows_several_paths());
        // an arriving entry of "motor" is filtered with those before
        logs.push("motor", create_entry(LogLevel::Info, "core", "later"));
        table.update_shown(&logs);
        assert!(table.shows_several_paths());
    }

    #[test]
    fn autoscroll_button_stays_visible_in_a_small_view() {
        let mut logs = Logs::default();
        for _ in 0..100 {
            logs.push("", create_entry(LogLevel::Info, "core", "message"));
        }
        let mut table = table();
        let time = TimeRange::default();
        let size = egui::vec2(400.0, 120.0);
        let harness = Harness::builder()
            .with_size(size)
            .build_ui(move |ui| table.ui(ui, &logs, &time));
        let button = harness.get_by_label(regular::ARROW_LINE_DOWN);
        assert!(button.rect().bottom() <= size.y);
    }

    #[test]
    fn autoscroll_keeps_up_with_new_entries() {
        let mut logs = Logs::default();
        let mut table = table();
        let time = TimeRange::default();
        let newest = std::rc::Rc::new(std::cell::Cell::new(0));
        let shared = newest.clone();
        let mut frame = 0;
        let mut harness = Harness::builder()
            .with_size(egui::vec2(600.0, 200.0))
            .build_ui(move |ui| {
                // the harness turns the animations off, which hid the lag
                ui.ctx().all_styles_mut(|style| {
                    style.scroll_animation = egui::style::ScrollAnimation::default();
                });
                frame += 1;
                if frame % 7 == 0 {
                    let message = format!("message {}", frame / 7);
                    logs.push("", create_entry(LogLevel::Info, "core", &message));
                    shared.set(frame / 7);
                }
                table.ui(ui, &logs, &time);
            });
        for step in 0..300 {
            harness.input_mut().time = Some(f64::from(step) / 60.0);
            harness.step();
        }
        let footer = harness.get_by_label(regular::ARROW_LINE_DOWN).rect();
        let newest = format!("message {}", newest.get());
        let entry = harness.get_by_label(&newest).rect();
        assert!(entry.bottom() <= footer.top());
    }

    #[test]
    fn the_header_holds_the_filters() {
        let logs = Logs::default();
        let mut table = table();
        let time = TimeRange::default();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(600.0, 300.0))
            .build_ui(move |ui| table.ui(ui, &logs, &time));
        harness
            .get_by_label(&format!("Level {}", regular::CARET_DOWN))
            .click();
        harness.run();
        harness.get_by_label("ERROR");
        harness.get_by_label("TRACE");
    }

    #[test]
    fn hidden_levels_are_filtered() {
        let mut table = table();
        let debug_entry = create_entry(LogLevel::Debug, "core", "debug message");
        let warn_entry = create_entry(LogLevel::Warn, "core", "warn message");

        assert!(table.matches(&debug_entry));
        assert!(table.matches(&warn_entry));

        table.filter.hidden_levels.insert(LogLevel::Debug);
        assert!(!table.matches(&debug_entry));
        assert!(table.matches(&warn_entry));
    }

    #[test]
    fn hidden_components_are_filtered() {
        let mut table = table();
        let entry1 = create_entry(LogLevel::Info, "motor_ctrl", "speed 100");
        let entry2 = create_entry(LogLevel::Info, "sensor_nav", "heading 90");

        table
            .filter
            .hidden_components
            .insert("sensor_nav".to_string());
        assert!(table.matches(&entry1));
        assert!(!table.matches(&entry2));

        // not while the components are hidden
        table.columns.component = false;
        assert!(table.matches(&entry2));
        assert!(!table.filter.is_active(false));
        table.columns.component = true;

        table.filter.clear();
        assert!(table.matches(&entry1));
        assert!(table.matches(&entry2));
    }

    #[test]
    fn the_columns_follow_the_properties() {
        let harness = |columns: LogColumns| {
            let mut logs = Logs::default();
            let entry = LogEntry {
                time: 1_700_000_000_000_000_000,
                ..create_entry(LogLevel::Info, "motor_ctrl", "speed 100")
            };
            logs.push("", entry);
            let mut table = LogTable { columns, ..table() };
            let time = TimeRange::default();
            Harness::builder()
                .with_size(egui::vec2(600.0, 300.0))
                .build_ui(move |ui| table.ui(ui, &logs, &time))
        };
        let shown = harness(LogColumns::default());
        shown.get_by_label("motor_ctrl");
        shown.get_by_label(&format!("Component {}", regular::CARET_DOWN));
        let date = TimeBase::Unix.format_dated(1_700_000_000_000_000_000, 3);
        assert!(shown.query_by_label(&date).is_none());

        let hidden = harness(LogColumns {
            date: true,
            component: false,
        });
        assert!(hidden.query_by_label("motor_ctrl").is_none());
        hidden.get_by_label(&date);
    }

    #[test]
    fn clearing_the_filters_keeps_autoscroll() {
        let mut filter = LogFilter {
            auto_scroll: false,
            message_regex: "x".to_string(),
            ..LogFilter::default()
        };
        assert!(filter.is_active(true));
        filter.clear();
        assert!(!filter.is_active(true));
        assert!(!filter.auto_scroll);
    }

    #[test]
    fn test_regex_filter() {
        let mut table = table();
        let entry1 = create_entry(
            LogLevel::Info,
            "ws",
            "connection established to ws://localhost",
        );
        let entry2 = create_entry(LogLevel::Warn, "ws", "failed connection retrying");

        table.filter.message_regex = r"^connection.*".to_string();
        table.update_regex_cache();
        assert!(table.matches(&entry1));
        assert!(!table.matches(&entry2));

        // Case sensitivity check
        table.filter.message_regex = r"ESTABLISHED".to_string();
        table.filter.case_sensitive = true;
        table.update_regex_cache();
        assert!(!table.matches(&entry1));

        table.filter.case_sensitive = false;
        table.update_regex_cache();
        assert!(table.matches(&entry1));

        // an invalid pattern is searched as text
        let entry3 = create_entry(LogLevel::Info, "ws", "retrying (3 left)");
        table.filter.message_regex = r"(3".to_string();
        table.update_regex_cache();
        assert!(matches!(table.compiled_regex.as_deref(), Some(Err(_))));
        assert!(table.matches(&entry3));
        assert!(!table.matches(&entry1));
    }
}
