//! Presets: named sets of the layout of the views and the connections, to switch
//! between, e.g. one per project or target. One preset is active: the layout and the
//! connections shown are its own, and changing them changes it. blip keeps the presets
//! across restarts, and exports them to and imports them from JSON files that hold one
//! or more presets.
//!
//! The kept presets and the files share one versioned format, [`PresetsFile`]: when it
//! changes, [`read`] migrates older versions.

use super::connections::Connection;
use super::layout::Group;

/// Version of the format of the presets, written by [`write`]. Version 2 adds the
/// number format Labels and the Fit to of plots and XY views; version 1 reads as it is.
pub const VERSION: u32 = 2;

/// Storage key of the presets, in the format of the files.
const PRESETS_KEY: &str = "presets";
/// Storage key of the name of the active preset.
const ACTIVE_KEY: &str = "active preset";
/// Storage key of kept presets that could not be read, kept aside, see
/// [`Presets::load`].
const UNREAD_KEY: &str = "unread presets";

/// The name of the preset blip starts with.
const DEFAULT_NAME: &str = "Default";

/// Why the active preset cannot change while recording.
const RECORDING_HINT: &str = "Stop recording to switch presets";

/// The width of the dialogs.
const DIALOG_WIDTH: f32 = 320.0;

/// A preset as it is kept and exported.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct Preset {
    /// The name of the preset, the names of the presets differ and are not blank.
    pub name: String,
    /// The connections, connected to none.
    #[serde(default)]
    pub connections: Vec<Connection>,
    /// The layout of the views: the group of the tabs at the top level.
    #[serde(default = "empty_layout")]
    pub layout: Group,
}

fn empty_layout() -> Group {
    Group::tabs(vec![])
}

impl Preset {
    /// A preset with an empty layout and the default connections.
    fn new(name: String) -> Self {
        Self {
            name,
            connections: Connection::defaults(),
            layout: empty_layout(),
        }
    }
}

/// The saved form of presets, in a file or kept by blip.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct PresetsFile {
    pub version: u32,
    pub presets: Vec<Preset>,
}

/// Just the version of a [`PresetsFile`], read before the rest, whose format depends
/// on it.
#[derive(serde::Deserialize)]
struct Versioned {
    version: u32,
    /// The tree of a layout file of the development versions before presets, which
    /// had a version too.
    #[serde(default)]
    root: Option<serde::de::IgnoredAny>,
}

impl Versioned {
    fn of(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }
}

/// The JSON of a presets file with `presets`.
pub fn write<'a>(presets: impl IntoIterator<Item = &'a Preset>) -> Vec<u8> {
    let file = PresetsFile {
        version: VERSION,
        presets: presets.into_iter().cloned().collect(),
    };
    serde_json::to_vec_pretty(&file).expect("presets are JSON")
}

/// Whether `bytes` look like a presets file: a JSON object with a version, whatever
/// else the format of that version holds, so that [`read`] can tell what is wrong.
pub fn is_presets(bytes: &[u8]) -> bool {
    Versioned::of(bytes).is_ok()
}

/// Reads the presets of the file `name` holding `bytes`, of this or an older version
/// of the format. They must have names, which differ; their connections are kept
/// apart as editing keeps them.
pub fn read(name: &str, bytes: &[u8]) -> Result<Vec<Preset>, String> {
    let Versioned { version, root } =
        Versioned::of(bytes).map_err(|error| format!("{name} is no blip presets file: {error}"))?;
    if root.is_some() {
        return Err(format!(
            "{name} is a layout file of an earlier version of blip, which presets replace"
        ));
    }
    if version > VERSION {
        return Err(format!("{name} holds presets of a newer version of blip"));
    }
    // older versions are migrated here, when there are any; version 1 needs no migration
    let file: PresetsFile = serde_json::from_slice(bytes)
        .map_err(|error| format!("{name} is no blip presets file: {error}"))?;
    let mut presets = file.presets;
    if presets.is_empty() {
        return Err(format!("{name} holds no presets"));
    }
    for index in 0..presets.len() {
        let trimmed = presets[index].name.trim().to_owned();
        if trimmed.is_empty() {
            return Err(format!("{name} holds a preset without a name"));
        }
        if presets[..index].iter().any(|preset| preset.name == trimmed) {
            return Err(format!("{name} holds two presets named {trimmed}"));
        }
        let preset = &mut presets[index];
        preset.name = trimmed;
        preset.connections = Connection::sanitized(std::mem::take(&mut preset.connections));
    }
    Ok(presets)
}

/// A change of the presets asked for in the menu or a dialog, which also changes what
/// is shown, applied by the app.
pub enum Action {
    /// Makes the preset at this index the active one.
    Switch(usize),
    /// Adds a preset with an empty layout and makes it the active one.
    New,
    /// Adds a copy of the active preset and makes it the active one.
    Duplicate,
    /// Deletes the active preset, a neighbour becomes the active one.
    Delete,
    /// Lets the user choose a presets file to import.
    Import,
    /// Saves the presets at these indices to a file.
    Export(Vec<usize>),
    /// Adds these presets, replacing those of the same names.
    Imported(Vec<Preset>),
}

/// The dialog being shown.
enum Dialog {
    Rename {
        name: String,
    },
    Delete,
    Export {
        /// Whether each preset is exported.
        chosen: Vec<bool>,
    },
    Import {
        file: String,
        presets: Vec<Preset>,
        /// Whether each preset of the file is imported.
        chosen: Vec<bool>,
    },
}

/// What a dialog asked for once it is shown.
enum Outcome {
    Open,
    Close,
    Apply(Action),
}

/// Which of the buttons of a dialog was clicked, see [`buttons_ui`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pressed {
    Nothing,
    Cancel,
    Apply,
}

/// Why the kept presets were not read, see [`Presets::load`].
enum Unread {
    /// They are of a newer version of blip, so they are left as they are.
    Newer,
    /// They are damaged, these are kept aside.
    Damaged(String),
}

pub struct Presets {
    /// The presets in the order they were added. The active one holds what it held
    /// when it was last stored, see [`Self::store`].
    list: Vec<Preset>,
    /// The index of the active preset in `list`.
    active: usize,
    dialog: Option<Dialog>,
    /// Why the kept presets were not read, if they were not.
    unread: Option<Unread>,
    /// What to tell the user about the kept presets, see [`Self::take_problem`].
    problem: Option<String>,
}

impl Default for Presets {
    fn default() -> Self {
        Self {
            list: vec![Preset::new(DEFAULT_NAME.to_owned())],
            active: 0,
            dialog: None,
            unread: None,
            problem: None,
        }
    }
}

impl Presets {
    /// The presets kept in `storage`, with the one that was active, or the default
    /// preset if there are none. Kept presets that cannot be read start the default
    /// preset too, without losing them: those of a newer version of blip are not
    /// overwritten by [`Self::save`], damaged ones are kept aside.
    pub fn load(storage: &dyn eframe::Storage) -> Self {
        let Some(saved) = storage.get_string(PRESETS_KEY) else {
            return Self::default();
        };
        let list = match read("The kept presets", saved.as_bytes()) {
            Ok(list) => list,
            Err(error) => {
                log::warn!("{error}, starting with the default preset");
                let newer = Versioned::of(saved.as_bytes())
                    .is_ok_and(|versioned| versioned.version > VERSION);
                let (unread, problem) = if newer {
                    let problem = "The kept presets are of a newer version of blip: \
                                   blip starts with the default preset and keeps them \
                                   as they are, changes to presets are not kept";
                    (Unread::Newer, problem.to_owned())
                } else {
                    let problem = format!(
                        "{error}: blip starts with the default preset and keeps the \
                         unread presets aside"
                    );
                    (Unread::Damaged(saved), problem)
                };
                return Self {
                    unread: Some(unread),
                    problem: Some(problem),
                    ..Self::default()
                };
            }
        };
        let active = storage.get_string(ACTIVE_KEY);
        let active = (list.iter())
            .position(|preset| Some(&preset.name) == active.as_ref())
            .unwrap_or(0);
        Self {
            list,
            active,
            ..Self::default()
        }
    }

    /// Keeps the presets in `storage`, as last stored, unless the kept ones are of a
    /// newer version of blip. Damaged kept presets are kept aside first.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        match &self.unread {
            Some(Unread::Newer) => return,
            Some(Unread::Damaged(json)) => storage.set_string(UNREAD_KEY, json.clone()),
            None => {}
        }
        let json = String::from_utf8(write(&self.list)).expect("JSON is UTF-8");
        storage.set_string(PRESETS_KEY, json);
        storage.set_string(ACTIVE_KEY, self.active().name.clone());
    }

    /// What to tell the user about the kept presets once, e.g. why they were not read.
    pub fn take_problem(&mut self) -> Option<String> {
        self.problem.take()
    }

    pub fn active(&self) -> &Preset {
        &self.list[self.active]
    }

    /// The names of the presets, in their order.
    #[cfg(test)]
    pub fn names(&self) -> Vec<&str> {
        self.list
            .iter()
            .map(|preset| preset.name.as_str())
            .collect()
    }

    /// Whether a dialog is shown.
    #[cfg(test)]
    pub fn has_dialog(&self) -> bool {
        self.dialog.is_some()
    }

    /// Stores the shown `layout` and `connections` in the active preset.
    pub fn store(&mut self, layout: Group, connections: Vec<Connection>) {
        let preset = &mut self.list[self.active];
        preset.layout = layout;
        preset.connections = connections;
    }

    /// Makes the preset at `index` the active one.
    pub fn switch(&mut self, index: usize) {
        if index < self.list.len() {
            self.active = index;
        }
    }

    /// Adds a preset with an empty layout and the default connections and makes it the
    /// active one.
    pub fn add_new(&mut self) {
        let name = self.unused_name("Preset", true);
        self.add(Preset::new(name));
    }

    /// Adds a copy of the active preset and makes it the active one.
    pub fn duplicate(&mut self) {
        let active = self.active();
        let name = self.unused_name(&format!("{} copy", active.name), false);
        let copy = Preset {
            name,
            ..active.clone()
        };
        self.add(copy);
    }

    fn add(&mut self, preset: Preset) {
        self.list.push(preset);
        self.active = self.list.len() - 1;
    }

    /// Deletes the active preset, unless it is the only one. The next one becomes the
    /// active one, the one before if it was the last.
    pub fn delete(&mut self) {
        if self.list.len() > 1 {
            self.list.remove(self.active);
            self.active = self.active.min(self.list.len() - 1);
        }
    }

    /// Adds `presets`, each replacing the preset of the same name. Returns whether the
    /// active preset was replaced.
    pub fn import(&mut self, presets: Vec<Preset>) -> bool {
        let mut active_replaced = false;
        for preset in presets {
            match self.index_of(&preset.name) {
                Some(index) => {
                    active_replaced |= index == self.active;
                    self.list[index] = preset;
                }
                None => self.list.push(preset),
            }
        }
        active_replaced
    }

    /// The presets at `indices`, in their order.
    pub fn export(&self, indices: &[usize]) -> Vec<u8> {
        write(indices.iter().filter_map(|&index| self.list.get(index)))
    }

    /// Shows the dialog to choose the presets of the file `file` to import, unless a
    /// dialog is shown, which it would replace.
    pub fn start_import(&mut self, file: String, presets: Vec<Preset>) -> Result<(), String> {
        if self.dialog.is_some() {
            return Err(format!(
                "{file} is not imported while a dialog of the presets is open"
            ));
        }
        let chosen = vec![true; presets.len()];
        self.dialog = Some(Dialog::Import {
            file,
            presets,
            chosen,
        });
        Ok(())
    }

    fn index_of(&self, name: &str) -> Option<usize> {
        self.list.iter().position(|preset| preset.name == name)
    }

    /// A name for a new preset that no preset has, see [`super::unused_name`].
    fn unused_name(&self, base: &str, numbered: bool) -> String {
        super::unused_name(base, numbered, |name| self.index_of(name).is_some())
    }

    /// Shows the presets to switch between and the menu items to manage them. While
    /// `recording`, the active preset stays.
    pub fn menu_ui(&mut self, ui: &mut egui::Ui, recording: bool) -> Option<Action> {
        let mut action = None;
        for (index, preset) in self.list.iter().enumerate() {
            let active = index == self.active;
            let button = egui::Button::selectable(active, preset.name.as_str());
            let clicked = ui
                .add_enabled(active || !recording, button)
                .on_disabled_hover_text(RECORDING_HINT)
                .clicked();
            if clicked && !active {
                action = Some(Action::Switch(index));
            }
        }
        ui.separator();
        let changing = |ui: &mut egui::Ui, text| {
            ui.add_enabled(!recording, egui::Button::new(text))
                .on_disabled_hover_text(RECORDING_HINT)
                .clicked()
        };
        if changing(ui, "New preset") {
            action = Some(Action::New);
        }
        if changing(ui, "Duplicate preset") {
            action = Some(Action::Duplicate);
        }
        if ui.button("Rename preset…").clicked() {
            let name = self.active().name.clone();
            self.dialog = Some(Dialog::Rename { name });
        }
        let deletable = !recording && self.list.len() > 1;
        if ui
            .add_enabled(deletable, egui::Button::new("Delete preset…"))
            .on_disabled_hover_text(if recording {
                RECORDING_HINT
            } else {
                "The only preset cannot be deleted"
            })
            .clicked()
        {
            self.dialog = Some(Dialog::Delete);
        }
        ui.separator();
        if ui.button("Import presets…").clicked() {
            action = Some(Action::Import);
        }
        if ui.button("Export presets…").clicked() {
            let chosen = vec![true; self.list.len()];
            self.dialog = Some(Dialog::Export { chosen });
        }
        action
    }

    /// Shows the open dialog, if any. While `recording`, the active preset stays.
    pub fn dialog_ui(&mut self, ctx: &egui::Context, recording: bool) -> Option<Action> {
        let mut dialog = self.dialog.take()?;
        let modal = egui::Modal::new(egui::Id::new("presets dialog")).show(ctx, |ui| {
            ui.set_width(DIALOG_WIDTH);
            match &mut dialog {
                Dialog::Rename { name } => self.rename_ui(ui, name),
                Dialog::Delete => self.delete_ui(ui, recording),
                Dialog::Export { chosen } => self.export_ui(ui, chosen),
                Dialog::Import {
                    file,
                    presets,
                    chosen,
                } => self.import_ui(ui, file, presets, chosen, recording),
            }
        });
        match modal.inner {
            Outcome::Apply(action) => Some(action),
            Outcome::Close => None,
            Outcome::Open if modal.should_close() => None,
            Outcome::Open => {
                self.dialog = Some(dialog);
                None
            }
        }
    }

    fn rename_ui(&mut self, ui: &mut egui::Ui, name: &mut String) -> Outcome {
        ui.heading("Rename preset");
        ui.add_space(4.0);
        let field = super::theme::wide_field(name);
        let response = ui.add(field);
        if !response.has_focus() && !response.lost_focus() {
            response.request_focus();
        }
        let trimmed = name.trim();
        let taken = self
            .index_of(trimmed)
            .is_some_and(|index| index != self.active);
        let problem = if trimmed.is_empty() {
            Some("A preset needs a name")
        } else if taken {
            Some("Another preset has this name")
        } else {
            None
        };
        ui.weak(problem.unwrap_or(" "));
        let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let mut pressed = buttons_ui(ui, "Rename", problem.is_none());
        if entered && problem.is_none() {
            pressed = Pressed::Apply;
        }
        match pressed {
            Pressed::Nothing => Outcome::Open,
            Pressed::Cancel => Outcome::Close,
            Pressed::Apply => {
                self.list[self.active].name = trimmed.to_owned();
                Outcome::Close
            }
        }
    }

    fn delete_ui(&self, ui: &mut egui::Ui, recording: bool) -> Outcome {
        ui.heading("Delete preset");
        ui.add_space(4.0);
        ui.label(format!(
            "Delete {} with its layout and connections?",
            self.active().name
        ));
        ui.add_space(4.0);
        outcome(buttons_ui(ui, "Delete", !recording), || Action::Delete)
    }

    fn export_ui(&self, ui: &mut egui::Ui, chosen: &mut [bool]) -> Outcome {
        ui.heading("Export presets");
        ui.add_space(4.0);
        ui.label("The presets to save to one file:");
        egui::ScrollArea::vertical()
            .max_height(240.0)
            .show(ui, |ui| {
                for (preset, chosen) in self.list.iter().zip(chosen.iter_mut()) {
                    ui.checkbox(chosen, preset.name.as_str());
                }
            });
        ui.add_space(4.0);
        let indices: Vec<usize> = (chosen.iter().enumerate())
            .filter_map(|(index, &chosen)| chosen.then_some(index))
            .collect();
        let pressed = buttons_ui(ui, "Export…", !indices.is_empty());
        outcome(pressed, || Action::Export(indices))
    }

    fn import_ui(
        &self,
        ui: &mut egui::Ui,
        file: &str,
        presets: &[Preset],
        chosen: &mut [bool],
        recording: bool,
    ) -> Outcome {
        ui.heading("Import presets");
        ui.add_space(4.0);
        ui.label(format!("The presets to import from {file}:"));
        // replacing the active preset changes the connections
        let locked =
            |preset: &Preset| recording && self.index_of(&preset.name) == Some(self.active);
        egui::ScrollArea::vertical()
            .max_height(240.0)
            .show(ui, |ui| {
                for (preset, chosen) in presets.iter().zip(chosen.iter_mut()) {
                    let existing = self.index_of(&preset.name);
                    let locked = locked(preset);
                    // a locked preset shows unchecked, and its choice comes back once
                    // recording stops
                    let mut shown = *chosen && !locked;
                    let checked = if locked { &mut shown } else { chosen };
                    ui.horizontal(|ui| {
                        ui.add_enabled(!locked, egui::Checkbox::new(checked, preset.name.as_str()))
                            .on_disabled_hover_text("Stop recording to replace the active preset");
                        if existing.is_some() {
                            ui.weak("replaces the existing one");
                        }
                    });
                }
            });
        ui.add_space(4.0);
        let taken: Vec<&Preset> = (presets.iter().zip(chosen.iter()))
            .filter(|&(preset, &chosen)| chosen && !locked(preset))
            .map(|(preset, _)| preset)
            .collect();
        let pressed = buttons_ui(ui, "Import", !taken.is_empty());
        outcome(pressed, || {
            Action::Imported(taken.into_iter().cloned().collect())
        })
    }
}

/// The buttons at the bottom of a dialog: Cancel and the one that applies it, named
/// `apply` and enabled if `enabled`.
fn buttons_ui(ui: &mut egui::Ui, apply: &str, enabled: bool) -> Pressed {
    let mut pressed = Pressed::Nothing;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.add_enabled(enabled, egui::Button::new(apply)).clicked() {
            pressed = Pressed::Apply;
        }
        if ui.button("Cancel").clicked() {
            pressed = Pressed::Cancel;
        }
    });
    pressed
}

/// The outcome of a dialog whose button `pressed`, that asks for `action` when
/// applied.
fn outcome(pressed: Pressed, action: impl FnOnce() -> Action) -> Outcome {
    match pressed {
        Pressed::Nothing => Outcome::Open,
        Pressed::Cancel => Outcome::Close,
        Pressed::Apply => Outcome::Apply(action()),
    }
}

#[cfg(test)]
mod tests;
