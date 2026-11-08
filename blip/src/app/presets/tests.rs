use super::*;
use crate::app::layout::Layout;
use crate::app::view::ViewType;
use eframe::Storage as _;

fn preset(name: &str) -> Preset {
    Preset::new(name.to_owned())
}

/// Why reading `bytes` as the file `a.json` fails.
fn refused(bytes: &[u8]) -> String {
    read("a.json", bytes)
        .err()
        .expect("the presets are refused")
}

/// A storage in memory, as eframe keeps it.
#[derive(Default)]
struct Memory(std::collections::HashMap<String, String>);

impl eframe::Storage for Memory {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }
    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }
    fn flush(&mut self) {}
}

#[test]
fn presets_survive_a_file() {
    let mut layout = Layout::default();
    layout.add_view(ViewType::Plot);
    let mut first = preset("Bench");
    first.layout = layout.to_saved();
    first.connections.push(Connection {
        name: "target".to_owned(),
        address: "ws://10.0.0.2:3000/ws/target".to_owned(),
    });
    let presets = [first, preset("Field")];
    let bytes = write(&presets);
    let read = read("presets.json", &bytes).unwrap();
    assert_eq!(write(&read), bytes);
    assert!(is_presets(&bytes));
    assert!(!is_presets(b"{\"presets\": []}"));
    assert!(!is_presets(b"not json"));
}

#[test]
fn the_demo_presets_are_read() {
    let bytes = include_bytes!("../../../tests/data/demo-presets.json");
    assert!(is_presets(bytes));
    let presets = read("demo-presets.json", bytes).unwrap();
    assert_eq!(presets[0].name, "Demo");
    assert!(!presets[0].layout.children.is_empty());
}

#[test]
fn presets_of_newer_versions_and_broken_names_are_refused() {
    let newer = format!(r#"{{"version": {}, "presets": []}}"#, VERSION + 1);
    assert_eq!(
        refused(newer.as_bytes()),
        "a.json holds presets of a newer version of blip"
    );
    let empty = br#"{"version": 1, "presets": []}"#;
    assert_eq!(refused(empty), "a.json holds no presets");
    let blank = br#"{"version": 1, "presets": [{"name": " "}]}"#;
    assert_eq!(refused(blank), "a.json holds a preset without a name");
    let twice = br#"{"version": 1, "presets": [{"name": "A"}, {"name": "A "}]}"#;
    assert_eq!(refused(twice), "a.json holds two presets named A");
}

#[test]
fn layout_files_of_earlier_versions_are_told_apart() {
    let layout = br#"{"version": 5, "root": {"name": "", "kind": "Tabs", "children": []}}"#;
    assert!(is_presets(layout));
    assert_eq!(
        refused(layout),
        "a.json is a layout file of an earlier version of blip, which presets replace"
    );
    // a newer file is told as such, whatever else it holds
    let newer = format!(r#"{{"version": {}, "sets": []}}"#, VERSION + 1);
    assert!(is_presets(newer.as_bytes()));
    assert!(refused(newer.as_bytes()).ends_with("of a newer version of blip"));
}

#[test]
fn the_connections_of_read_presets_are_kept_apart() {
    let file = br#"{"version": 1, "presets": [{"name": "A", "connections": [
        {"name": "a", "address": "ws://a"},
        {"name": "b", "address": " ws://a "},
        {"name": "a", "address": "ws://b"},
        {"address": ""}
    ]}]}"#;
    let presets = read("a.json", file).unwrap();
    let connections: Vec<(&str, &str)> = (presets[0].connections.iter())
        .map(|c| (c.name.as_str(), c.address.as_str()))
        .collect();
    assert_eq!(connections, [("a", "ws://a"), ("", "ws://b")]);
}

#[test]
fn importing_while_a_dialog_is_open_is_refused() {
    let mut presets = Presets::default();
    assert!(
        presets
            .start_import("a.json".to_owned(), vec![preset("A")])
            .is_ok()
    );
    let error = presets.start_import("b.json".to_owned(), vec![preset("B")]);
    assert_eq!(
        error.unwrap_err(),
        "b.json is not imported while a dialog of the presets is open"
    );
}

/// The presets, whether recording, and the names of the presets imported.
type ImportState = (Presets, bool, Option<Vec<String>>);

/// A harness showing the dialog to import the default preset and a new one.
fn importing(recording: bool) -> egui_kittest::Harness<'static, ImportState> {
    let mut presets = Presets::default();
    let file = vec![preset(DEFAULT_NAME), preset("New")];
    presets.start_import("a.json".to_owned(), file).unwrap();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(600.0, 400.0))
        .build_ui_state(
            |ui, (presets, recording, imported): &mut ImportState| {
                if let Some(Action::Imported(taken)) = presets.dialog_ui(ui.ctx(), *recording) {
                    *imported = Some(taken.into_iter().map(|p| p.name).collect());
                }
            },
            (presets, recording, None),
        );
    harness.run();
    harness
}

#[test]
fn the_active_preset_is_not_replaced_while_recording() {
    use egui_kittest::kittest::Queryable;

    let mut harness = importing(true);
    harness.get_by_label("Import").click();
    harness.run();
    assert_eq!(harness.state().2, Some(vec!["New".to_owned()]));

    // stopping recording brings the choice back
    let mut harness = importing(true);
    harness.state_mut().1 = false;
    harness.run();
    harness.get_by_label("Import").click();
    harness.run();
    let both = vec![DEFAULT_NAME.to_owned(), "New".to_owned()];
    assert_eq!(harness.state().2, Some(both));
}

#[test]
fn a_generated_preset_needs_only_a_name() {
    let presets = read("a.json", br#"{"version": 1, "presets": [{"name": "A"}]}"#).unwrap();
    assert!(presets[0].connections.is_empty());
    assert!(presets[0].layout.children.is_empty());
}

#[test]
fn presets_are_kept_with_the_active_one() {
    let mut storage = Memory::default();
    assert_eq!(Presets::load(&storage).names(), [DEFAULT_NAME]);

    let mut presets = Presets::default();
    presets.add_new();
    presets.add_new();
    presets.switch(1);
    presets.save(&mut storage);
    let loaded = Presets::load(&storage);
    assert_eq!(loaded.names(), [DEFAULT_NAME, "Preset 1", "Preset 2"]);
    assert_eq!(loaded.active().name, "Preset 1");
}

#[test]
fn kept_presets_that_cannot_be_read_are_not_lost() {
    // those of a newer blip are left as they are
    let mut storage = Memory::default();
    let newer = format!(r#"{{"version": {}, "presets": []}}"#, VERSION + 1);
    storage.set_string(PRESETS_KEY, newer.clone());
    let mut presets = Presets::load(&storage);
    assert_eq!(presets.names(), [DEFAULT_NAME]);
    assert!(presets.take_problem().unwrap().contains("newer version"));
    assert!(presets.take_problem().is_none());
    presets.add_new();
    presets.save(&mut storage);
    assert_eq!(storage.get_string(PRESETS_KEY), Some(newer));

    // damaged ones are kept aside
    let mut storage = Memory::default();
    storage.set_string(PRESETS_KEY, "{".to_owned());
    let mut presets = Presets::load(&storage);
    assert!(
        presets
            .take_problem()
            .unwrap()
            .contains("unread presets aside")
    );
    presets.save(&mut storage);
    assert_eq!(storage.get_string(UNREAD_KEY).as_deref(), Some("{"));
    assert_eq!(Presets::load(&storage).names(), [DEFAULT_NAME]);
}

#[test]
fn new_and_duplicated_presets_get_unused_names_and_become_active() {
    let mut presets = Presets::default();
    presets.add_new();
    assert_eq!(presets.active().name, "Preset 1");
    presets.duplicate();
    assert_eq!(presets.active().name, "Preset 1 copy");
    presets.switch(1);
    presets.duplicate();
    assert_eq!(presets.active().name, "Preset 1 copy 2");
    assert_eq!(presets.list.len(), 4);
}

#[test]
fn deleting_makes_a_neighbour_active_but_keeps_the_last_preset() {
    let mut presets = Presets::default();
    presets.add_new();
    presets.add_new();
    presets.switch(1);
    presets.delete();
    assert_eq!(presets.names(), [DEFAULT_NAME, "Preset 2"]);
    assert_eq!(presets.active().name, "Preset 2");
    presets.delete();
    assert_eq!(presets.active().name, DEFAULT_NAME);
    presets.delete();
    assert_eq!(presets.names(), [DEFAULT_NAME]);
}

#[test]
fn importing_replaces_presets_of_the_same_name() {
    let mut presets = Presets::default();
    presets.add_new();
    presets.switch(0);
    let mut replacement = preset("Preset 1");
    replacement.connections.clear();
    assert!(!presets.import(vec![replacement, preset("New")]));
    assert_eq!(presets.names(), [DEFAULT_NAME, "Preset 1", "New"]);
    assert!(presets.list[1].connections.is_empty());
    assert_eq!(presets.active().name, DEFAULT_NAME);

    assert!(presets.import(vec![preset(DEFAULT_NAME)]));
}

#[test]
fn exports_hold_the_chosen_presets() {
    let mut presets = Presets::default();
    presets.add_new();
    presets.add_new();
    let exported = read("a.json", &presets.export(&[0, 2])).unwrap();
    let exported: Vec<&str> = exported.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(exported, [DEFAULT_NAME, "Preset 2"]);
}
