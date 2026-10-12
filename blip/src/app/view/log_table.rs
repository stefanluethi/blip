use std::collections::BTreeSet;
use egui_extras::{Column, TableBuilder};
use crate::app::model::logs::{LogEntry, LogLevel, Logs};
use crate::app::view::Viewable;

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(default)]
pub struct LogFilter {
    pub min_level: LogLevel,
    pub component: String,
    pub message_regex: String,
    pub case_sensitive: bool,
    pub auto_scroll: bool,
}

impl Default for LogFilter {
    fn default() -> Self {
        Self {
            min_level: LogLevel::Trace,
            component: String::new(),
            message_regex: String::new(),
            case_sensitive: false,
            auto_scroll: true,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)]
pub struct LogTable {
    pub filter: LogFilter,

    #[serde(skip)]
    compiled_regex: Option<Result<regex::Regex, String>>,
    #[serde(skip)]
    last_regex_str: String,
    #[serde(skip)]
    last_case_sensitive: bool,
    #[serde(skip)]
    filter_menu_open: bool,
}

impl LogTable {
    pub fn new() -> Self {
        Self::default()
    }

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
        // 1. Severity filter (minimum log level)
        if entry.level < self.filter.min_level {
            return false;
        }

        // 2. Component filter (exact match from dropdown, empty = all components)
        if !self.filter.component.is_empty() && entry.component != self.filter.component {
            return false;
        }

        // 3. Message Regex filter
        if !self.filter.message_regex.is_empty() {
            if let Some(ref res) = self.compiled_regex {
                match res {
                    Ok(re) => {
                        if !re.is_match(&entry.message) {
                            return false;
                        }
                    }
                    Err(_) => {
                        // Fallback case-insensitive or case-sensitive substring matching if regex pattern syntax is invalid/incomplete
                        let query = if self.filter.case_sensitive {
                            self.filter.message_regex.clone()
                        } else {
                            self.filter.message_regex.to_lowercase()
                        };
                        let target = if self.filter.case_sensitive {
                            entry.message.clone()
                        } else {
                            entry.message.to_lowercase()
                        };
                        if !target.contains(&query) {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, logs: &Logs) {
        self.update_regex_cache();

        let filtered_entries: Vec<&LogEntry> = logs
            .entries
            .iter()
            .filter(|entry| self.matches(entry))
            .collect();

        self.show_controls(ui, logs);

        ui.add_space(4.0);

        let footer_height = 24.0;
        let table_height = (ui.available_height() - footer_height).max(50.0);

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), table_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                let text_height = egui::TextStyle::Monospace.resolve(ui.style()).size;

                let mut builder = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::initial(80.0).at_least(80.0))  // Timestamp
                    .column(Column::initial(100.0).at_least(40.0)) // Log Component
                    .column(Column::initial(50.0).at_least(30.0))   // Log Level
                    .column(Column::remainder().at_least(150.0));   // Message

                if self.filter.auto_scroll && !filtered_entries.is_empty() {
                    builder = builder.scroll_to_row(filtered_entries.len() - 1, Some(egui::Align::Max));
                }

                builder.body(|body| {
                    body.rows(text_height + 6.0, filtered_entries.len(), |mut row| {
                        let entry = filtered_entries[row.index()];
                        row.col(|ui| {
                            ui.label(egui::RichText::new(&entry.timestamp).monospace());
                        });
                        row.col(|ui| {
                            ui.label(egui::RichText::new(&entry.component).monospace());
                        });
                        row.col(|ui| {
                            let (color, level_str) = match entry.level {
                                LogLevel::Error => (egui::Color32::from_rgb(235, 87, 87), "ERROR"),
                                LogLevel::Warn => (egui::Color32::from_rgb(242, 153, 74), "WARN"),
                                LogLevel::Info => (egui::Color32::from_rgb(39, 174, 96), "INFO"),
                                LogLevel::Debug => (egui::Color32::from_rgb(45, 156, 219), "DEBUG"),
                                LogLevel::Trace => (egui::Color32::from_rgb(155, 81, 224), "TRACE"),
                            };
                            ui.colored_label(color, egui::RichText::new(level_str).monospace());
                        });
                        row.col(|ui| {
                            ui.label(egui::RichText::new(&entry.message).monospace());
                        });
                    });
                });
            },
        );

        ui.add_space(2.0);

        // Footer showing entries count below table
        egui::Frame::default()
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let auto_scroll_btn = egui::Button::new(
                        egui_phosphor::regular::ARROW_LINE_DOWN)
                        .selected(self.filter.auto_scroll);

                    if ui
                        .add(auto_scroll_btn)
                        .on_hover_text("Toggle autoscroll")
                        .clicked()
                    {
                        self.filter.auto_scroll = !self.filter.auto_scroll;
                    }


                    ui.label(
                        egui::RichText::new(format!(
                            "Showing {} / {} entries",
                            filtered_entries.len(),
                            logs.entries.len()
                        ))
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                });
            });
    }

    fn show_controls(
        &mut self,
        ui: &mut egui::Ui,
        logs: &Logs,
    ) {
        egui::Frame::default()
            .fill(ui.visuals().faint_bg_color)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Top Bar: Search, Filter Toggle Button, Stats
                    ui.horizontal(|ui| {
                        // 1. Message Regex Search Bar
                        let is_regex_err = matches!(self.compiled_regex, Some(Err(_)));

                        ui.add(
                            egui::TextEdit::singleline(&mut self.filter.message_regex)
                                .hint_text(format!("{} Search logs (regex)...", egui_phosphor::regular::MAGNIFYING_GLASS))
                                .font(egui::TextStyle::Monospace)
                                .desired_width(200.0),
                        );

                        // Case sensitivity toggle button
                        let cs_text = egui::RichText::new("Aa").monospace();
                        let cs_btn =
                            egui::Button::new(cs_text).selected(self.filter.case_sensitive);
                        if ui.add(cs_btn).on_hover_text("Toggle Case Sensitivity").clicked() {
                            self.filter.case_sensitive = !self.filter.case_sensitive;
                        }

                        if is_regex_err {
                            if let Some(Err(ref err_msg)) = self.compiled_regex {
                                ui.label(egui::RichText::new(egui_phosphor::regular::WARNING).color(egui::Color32::RED))
                                    .on_hover_text(format!("Invalid Regex Syntax:\n{}", err_msg));
                            }
                        }

                        if !self.filter.message_regex.is_empty()
                            && ui.small_button(egui_phosphor::regular::X).clicked()
                        {
                            self.filter.message_regex.clear();
                        }

                        ui.separator();

                        // 2. Filter Dropdown Toggle Button
                        let active_dropdown_filters = (self.filter.min_level != LogLevel::Trace) as usize
                            + (!self.filter.component.is_empty()) as usize;

                        let filter_icon = egui_phosphor::regular::FUNNEL;
                        let caret_icon = if self.filter_menu_open {
                            egui_phosphor::regular::CARET_UP
                        } else {
                            egui_phosphor::regular::CARET_DOWN
                        };

                        let filter_title = if active_dropdown_filters > 0 {
                            format!(
                                "{} Filter ({}) {}",
                                filter_icon,
                                active_dropdown_filters,
                                caret_icon
                            )
                        } else {
                            format!("{} Filter {}", filter_icon, caret_icon)
                        };

                        let filter_btn =
                            egui::Button::new(&filter_title).selected(self.filter_menu_open);
                        if ui.add(filter_btn).clicked() {
                            self.filter_menu_open = !self.filter_menu_open;
                        }

                        // 3. Right-aligned Controls
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {


                            let is_filtered = self.filter.min_level != LogLevel::Trace
                                || !self.filter.component.is_empty()
                                || !self.filter.message_regex.is_empty()
                                || self.filter.case_sensitive;

                            if is_filtered && ui.button("Reset").clicked() {
                                self.filter = LogFilter::default();
                            }
                        });
                    });

                    // Expandable Filter Panel when filter_menu_open is true
                    if self.filter_menu_open {
                        ui.add_space(6.0);

                        egui::Frame::default()
                            .fill(ui.visuals().widgets.noninteractive.bg_fill)
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::same(8))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    // Severity Row
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Severity:").strong());
                                        ui.selectable_value(
                                            &mut self.filter.min_level,
                                            LogLevel::Trace,
                                            "TRACE (All)",
                                        );
                                        ui.selectable_value(
                                            &mut self.filter.min_level,
                                            LogLevel::Debug,
                                            "DEBUG+",
                                        );
                                        ui.selectable_value(
                                            &mut self.filter.min_level,
                                            LogLevel::Info,
                                            "INFO+",
                                        );
                                        ui.selectable_value(
                                            &mut self.filter.min_level,
                                            LogLevel::Warn,
                                            "WARN+",
                                        );
                                        ui.selectable_value(
                                            &mut self.filter.min_level,
                                            LogLevel::Error,
                                            "ERROR",
                                        );
                                    });

                                    ui.add_space(6.0);

                                    // Component Row
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Component:").strong());
                                        let unique_components: BTreeSet<&str> = logs
                                            .entries
                                            .iter()
                                            .map(|e| e.component.as_str())
                                            .collect();

                                        ui.horizontal_wrapped(|ui| {
                                            if ui
                                                .selectable_label(
                                                    self.filter.component.is_empty(),
                                                    "All",
                                                )
                                                .clicked()
                                            {
                                                self.filter.component.clear();
                                            }
                                            for comp in unique_components {
                                                let is_selected =
                                                    self.filter.component == comp;
                                                if ui
                                                    .selectable_label(is_selected, comp)
                                                    .clicked()
                                                {
                                                    self.filter.component = comp.to_string();
                                                }
                                            }
                                        });
                                    });
                                });
                            });
                    }
                });
            });
    }

}

impl Viewable for LogTable {
    fn view(&mut self, ui: &mut egui::Ui, model: &mut crate::app::model::Model) {
        self.ui(ui, &model.logs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_entry(level: LogLevel, component: &str, message: &str) -> LogEntry {
        LogEntry {
            timestamp: "12:00:00.000".to_string(),
            component: component.to_string(),
            level,
            message: message.to_string(),
        }
    }

    #[test]
    fn test_severity_filter() {
        let mut table = LogTable::default();
        let trace_entry = create_entry(LogLevel::Trace, "core", "trace message");
        let debug_entry = create_entry(LogLevel::Debug, "core", "debug message");
        let warn_entry = create_entry(LogLevel::Warn, "core", "warn message");
        let error_entry = create_entry(LogLevel::Error, "core", "error message");

        // Default: min_level = Trace (All)
        assert!(table.matches(&trace_entry));
        assert!(table.matches(&debug_entry));
        assert!(table.matches(&warn_entry));
        assert!(table.matches(&error_entry));

        // Set min_level = Warn
        table.filter.min_level = LogLevel::Warn;
        assert!(!table.matches(&trace_entry));
        assert!(!table.matches(&debug_entry));
        assert!(table.matches(&warn_entry));
        assert!(table.matches(&error_entry));
    }

    #[test]
    fn test_component_filter() {
        let mut table = LogTable::default();
        let entry1 = create_entry(LogLevel::Info, "motor_ctrl", "speed 100");
        let entry2 = create_entry(LogLevel::Info, "sensor_nav", "heading 90");

        // Selected component
        table.filter.component = "motor_ctrl".to_string();
        assert!(table.matches(&entry1));
        assert!(!table.matches(&entry2));

        // All components (empty string)
        table.filter.component.clear();
        assert!(table.matches(&entry1));
        assert!(table.matches(&entry2));
    }

    #[test]
    fn test_regex_filter() {
        let mut table = LogTable::default();
        let entry1 = create_entry(LogLevel::Info, "ws", "connection established to ws://localhost");
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
    }
}
