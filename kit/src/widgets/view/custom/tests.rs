use super::*;
use crate::widgets::view::{View, ViewKind, ViewType};

/// A view type of an application: it shows the traces picked for it and keeps a
/// setting.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
struct Gauge {
    traces: BTreeSet<String>,
    #[serde(default)]
    max: f64,
}

impl Viewable for Gauge {
    fn view(&mut self, ui: &mut egui::Ui, _context: &mut ViewContext<'_>) {
        ui.label(format!("{} traces up to {}", self.traces.len(), self.max));
    }

    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        Some((&mut self.traces, false))
    }
}

impl ViewInfo for Gauge {
    const NAME: &'static str = "Gauge";
    const ICON: &'static str = egui_phosphor::regular::GAUGE;
    const SHOWS_ONE_TRACE: bool = true;

    fn selectable(_trace: Trace<'_>) -> Selectable {
        Selectable::Trace
    }
}

/// A type with the name of one of the library.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
struct Plot;

impl Viewable for Plot {
    fn view(&mut self, _ui: &mut egui::Ui, _context: &mut ViewContext<'_>) {}
}

impl ViewInfo for Plot {
    const NAME: &'static str = "Plot";
    const ICON: &'static str = "";

    fn selectable(_trace: Trace<'_>) -> Selectable {
        Selectable::Trace
    }
}

fn gauge_type() -> ViewType {
    register::<Gauge>();
    ViewType::all()
        .into_iter()
        .find(|view_type| view_type.name() == "Gauge")
        .expect("the registered type")
}

#[test]
fn a_registered_type_is_offered_after_those_of_the_library() {
    let gauge = gauge_type();
    let all = ViewType::all();
    assert_eq!(all[..ViewType::BUILT_IN.len()], ViewType::BUILT_IN);
    assert!(all[ViewType::BUILT_IN.len()..].contains(&gauge));
    assert_eq!(gauge.icon(), egui_phosphor::regular::GAUGE);
    assert!(gauge.has_selection());
    assert!(gauge.shows_one_trace());
    assert!(!gauge.shows_logs());
}

#[test]
fn registering_twice_or_under_a_name_of_the_library_changes_nothing() {
    gauge_type();
    register::<Gauge>();
    register::<Plot>();
    let names: Vec<&str> = ViewType::all().iter().map(|t| t.name()).collect();
    assert_eq!(names.iter().filter(|name| **name == "Gauge").count(), 1);
    assert_eq!(names.iter().filter(|name| **name == "Plot").count(), 1);
}

#[test]
fn a_view_of_a_registered_type_is_saved_and_loaded_with_its_state() {
    let mut view = View::new(gauge_type(), "Tank".to_owned());
    let (traces, _) = view.kind.viewable().selection().expect("a selection");
    traces.insert("tank/level".to_owned());

    let saved = serde_json::to_value(&view).expect("a saved view");
    assert_eq!(
        saved["kind"],
        serde_json::json!({"Custom": {"type": "Gauge", "state": {"traces": ["tank/level"], "max": 0.0}}})
    );

    let mut loaded: View = serde_json::from_value(saved).expect("a view");
    assert_eq!(loaded.name, "Tank");
    assert_eq!(loaded.view_type(), gauge_type());
    let (traces, _) = loaded.kind.viewable().selection().expect("a selection");
    assert!(traces.contains("tank/level"));
    // a copy of a view keeps the state too
    let mut copy = loaded.clone();
    let (traces, _) = copy.kind.viewable().selection().expect("a selection");
    assert!(traces.contains("tank/level"));
}

#[test]
fn a_view_of_a_type_that_is_not_registered_is_kept_as_saved() {
    let saved = serde_json::json!({
        "name": "Elsewhere",
        "kind": {"Custom": {"type": "Not here", "state": {"speed": 3}}},
    });
    let view: View = serde_json::from_value(saved.clone()).expect("a view");
    assert_eq!(view.view_type().name(), "Unknown");
    assert!(!view.view_type().has_selection());
    assert!(matches!(view.kind, ViewKind::Custom(_)));
    let again = serde_json::to_value(&view).expect("a saved view");
    assert_eq!(again["kind"], saved["kind"]);
}

#[test]
fn a_view_of_a_registered_type_with_a_damaged_state_is_refused() {
    gauge_type();
    let saved = serde_json::json!({
        "name": "Tank",
        "kind": {"Custom": {"type": "Gauge", "state": {"traces": 3}}},
    });
    assert!(serde_json::from_value::<View>(saved).is_err());
}

#[test]
fn views_of_the_library_are_saved_as_before() {
    let view = View::new(ViewType::Table, "Table".to_owned());
    let saved = serde_json::to_value(&view).expect("a saved view");
    assert!(saved["kind"].get("Table").is_some());
}
