use crate::app::model::Version;
use crate::app::model::logs::{LogEntry, LogLevel, Logs};
use crate::app::theme;
use crate::app::time::{TimeBase, TimeRange};
use crate::app::view::{ViewContext, Viewable};
use egui_extras::Column;
use egui_phosphor::regular;
use std::collections::BTreeSet;

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
    /// Whether any entries are hidden by the filters.
    fn is_active(&self) -> bool {
        !self.hidden_levels.is_empty()
            || !self.hidden_components.is_empty()
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

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct LogTable {
    pub filter: LogFilter,

    #[serde(skip)]
    compiled_regex: Option<Result<regex::Regex, String>>,
    #[serde(skip)]
    last_regex_str: String,
    #[serde(skip)]
    last_case_sensitive: bool,
    /// The time range shown in the last frame, to scroll to it when it moves.
    #[serde(skip)]
    last_range: Option<(f64, f64)>,
    /// The indices of the entries the filter shows, see [`LogTable::update_shown`].
    #[serde(skip)]
    shown: Vec<u32>,
    /// What `shown` was made for: a hash of what the filter hides, the order of the
    /// entries and how many of them were filtered.
    #[serde(skip)]
    shown_for: Option<Box<(u64, Version, usize)>>,
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
                self.compiled_regex = Some(res);
            }
        }
    }

    pub fn matches(&self, entry: &LogEntry) -> bool {
        if self.filter.hidden_levels.contains(&entry.level)
            || self.filter.hidden_components.contains(&entry.component)
        {
            return false;
        }
        match &self.compiled_regex {
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

    /// Filters the entries of `logs` that arrived since the last call, or all of them
    /// again if the filter or the order of the entries changed.
    fn update_shown(&mut self, logs: &Logs) {
        use std::hash::{Hash, Hasher};
        self.update_regex_cache();
        // without whether to scroll, which shows the same entries
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let filter = &self.filter;
        (&filter.hidden_levels, &filter.hidden_components).hash(&mut hasher);
        (&filter.message_regex, filter.case_sensitive).hash(&mut hasher);
        let filter = hasher.finish();
        let from = match self.shown_for.take().map(|known| *known) {
            Some((known, order, filtered)) if known == filter && order == logs.order() => filtered,
            _ => {
                self.shown.clear();
                0
            }
        };
        for (index, entry) in logs.entries.iter().enumerate().skip(from) {
            if self.matches(entry) {
                self.shown.push(index as u32);
            }
        }
        self.shown_for = Some(Box::new((filter, logs.order(), logs.entries.len())));
    }

    /// Shows the entries, the ones outside the shown time range greyed out, with the
    /// filters in the header of the table.
    pub fn ui(&mut self, ui: &mut egui::Ui, logs: &Logs, time: &TimeRange) {
        self.update_shown(logs);

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

                // the time, level and component take only what they need, the message
                // the rest
                let mut builder = theme::table(ui)
                    .id_salt("log entries")
                    .min_scrolled_height(0.0)
                    // the footer stays at the bottom, also with few entries
                    .auto_shrink([false, false])
                    .column(Column::auto().resizable(false)) // Timestamp
                    .column(Column::auto().resizable(false)) // Log Level
                    .column(Column::auto().resizable(false)) // Log Component
                    .column(Column::remainder().at_least(150.0)); // Message

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
                        .partition_point(|&index| logs.entries[index as usize].time <= end)
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
                        header.col(|ui| {
                            self.component_filter_ui(ui, logs);
                        });
                        header.col(|ui| {
                            self.message_filter_ui(ui);
                        });
                    })
                    .body(|mut body| {
                        let lines = lines.in_body(body.ui_mut());
                        let shown_entries = &self.shown;
                        body.rows(row_height, shown_entries.len(), |mut row| {
                            let index = row.index();
                            let entry = &logs.entries[shown_entries[index] as usize];
                            let shown = in_range(entry);
                            // the message stands out, the time and the component less,
                            // and all of an entry outside the time range stays faint
                            let text = |text: &str, ui: &egui::Ui, role: Role| {
                                let tokens = theme::tokens(ui.visuals());
                                let color = match role {
                                    _ if !shown => tokens.text_faint,
                                    Role::Time => tokens.text_faint,
                                    Role::Component => ui.visuals().weak_text_color(),
                                    Role::Message => ui.visuals().text_color(),
                                };
                                egui::RichText::new(text).monospace().color(color)
                            };
                            row.col(|ui| {
                                lines.above(ui, index);
                                let time = TimeBase::guess(entry.time).format(entry.time, 3);
                                ui.label(text(&time, ui, Role::Time));
                            });
                            row.col(|ui| {
                                level_badge(ui, entry.level, shown);
                            });
                            row.col(|ui| {
                                ui.label(text(&entry.component, ui, Role::Component));
                            });
                            row.col(|ui| {
                                ui.label(text(&entry.message, ui, Role::Message));
                            });
                        });
                    });
            },
        );

        self.footer_ui(ui, self.shown.len(), logs.entries.len());
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

    /// The header of the component column, with checkboxes for the shown components.
    fn component_filter_ui(&mut self, ui: &mut egui::Ui, logs: &Logs) {
        let hidden = &mut self.filter.hidden_components;
        filter_header(ui, "Component", !hidden.is_empty(), |ui| {
            let components: BTreeSet<&str> = logs
                .entries
                .iter()
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
        let error = match &self.compiled_regex {
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
    const ICON: &'static str = egui_phosphor::regular::LIST_BULLETS;
    const HAS_SELECTION: bool = false;
    const WRITES_VALUES: bool = false;

    fn selectable(_trace: crate::app::model::traces::Trace<'_>) -> super::Selectable {
        super::Selectable::No("Log views show no traces".into())
    }
}

impl Viewable for LogTable {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        self.ui(ui, &context.model.logs, context.time);
    }
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
    Component,
    Message,
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    fn create_entry(level: LogLevel, component: &str, message: &str) -> LogEntry {
        LogEntry {
            time: 0,
            component: component.to_string(),
            level,
            message: message.to_string(),
        }
    }

    #[test]
    fn new_entries_are_filtered_and_all_again_when_the_filter_or_order_changes() {
        let at = |time, level| LogEntry {
            time,
            ..create_entry(level, "core", "message")
        };
        let mut logs = Logs::default();
        logs.push(at(1, LogLevel::Info));
        logs.push(at(2, LogLevel::Warn));
        let mut table = LogTable::default();
        table.filter.hidden_levels.insert(LogLevel::Info);
        table.update_shown(&logs);
        assert_eq!(table.shown, [1]);

        logs.push(at(3, LogLevel::Warn));
        logs.push(at(4, LogLevel::Info));
        table.update_shown(&logs);
        assert_eq!(table.shown, [1, 2]);

        table.filter.hidden_levels.clear();
        table.update_shown(&logs);
        assert_eq!(table.shown, [0, 1, 2, 3]);

        // an entry before the others moves them on
        table.filter.hidden_levels.insert(LogLevel::Info);
        table.update_shown(&logs);
        logs.push(at(0, LogLevel::Warn));
        table.update_shown(&logs);
        assert_eq!(table.shown, [0, 2, 3]);
    }

    #[test]
    fn autoscroll_button_stays_visible_in_a_small_view() {
        let mut logs = Logs::default();
        for _ in 0..100 {
            logs.push(create_entry(LogLevel::Info, "core", "message"));
        }
        let mut table = LogTable::default();
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
        let mut table = LogTable::default();
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
                    logs.push(create_entry(LogLevel::Info, "core", &message));
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
        let mut table = LogTable::default();
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
        let mut table = LogTable::default();
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
        let mut table = LogTable::default();
        let entry1 = create_entry(LogLevel::Info, "motor_ctrl", "speed 100");
        let entry2 = create_entry(LogLevel::Info, "sensor_nav", "heading 90");

        table
            .filter
            .hidden_components
            .insert("sensor_nav".to_string());
        assert!(table.matches(&entry1));
        assert!(!table.matches(&entry2));

        table.filter.clear();
        assert!(table.matches(&entry1));
        assert!(table.matches(&entry2));
    }

    #[test]
    fn clearing_the_filters_keeps_autoscroll() {
        let mut filter = LogFilter {
            auto_scroll: false,
            message_regex: "x".to_string(),
            ..LogFilter::default()
        };
        assert!(filter.is_active());
        filter.clear();
        assert!(!filter.is_active());
        assert!(!filter.auto_scroll);
    }

    #[test]
    fn test_regex_filter() {
        let mut table = LogTable::default();
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
        assert!(matches!(table.compiled_regex, Some(Err(_))));
        assert!(table.matches(&entry3));
        assert!(!table.matches(&entry1));
    }
}
