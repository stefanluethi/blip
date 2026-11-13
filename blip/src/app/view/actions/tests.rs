use super::*;
use crate::app::model::actions::DRAG_INTERVAL;
use egui_kittest::kittest::{NodeT, Queryable};

fn param(kind: ParamKind) -> Param {
    Param {
        name: "p".to_owned(),
        kind,
        unit: String::new(),
        default: None,
    }
}

#[test]
fn values_fit_the_kind_and_bounds_of_parameters() {
    let int = param(ParamKind::Int {
        min: Some(-5),
        max: Some(5),
    });
    assert_eq!(fit(&int, &Data::Float(2.6)), Some(Data::Uint(3)));
    assert_eq!(fit(&int, &Data::Int(-9)), Some(Data::Int(-5)));
    assert_eq!(fit(&int, &Data::from("x")), None);
    let choice = param(ParamKind::Enum(vec!["a".to_owned(), "b".to_owned()]));
    assert_eq!(fit(&choice, &Data::from("b")), Some(Data::from("b")));
    assert_eq!(fit(&choice, &Data::from("c")), None);
    assert_eq!(fit(&param(ParamKind::Bool), &Data::Uint(1)), None);
}

#[test]
fn integers_fit_exactly_and_bounds_the_wrong_way_round_do_not_panic() {
    let unbounded = param(ParamKind::Int {
        min: None,
        max: None,
    });
    let big = Data::Uint((1 << 53) + 1);
    assert_eq!(fit(&unbounded, &big), Some(big.clone()));
    assert_eq!(
        fit(&unbounded, &Data::Uint(u64::MAX)),
        Some(Data::Uint(u64::MAX))
    );
    let reversed = param(ParamKind::Int {
        min: Some(5),
        max: Some(1),
    });
    assert!(fit(&reversed, &Data::Uint(3)).is_some());
    let reversed = param(ParamKind::Float {
        min: Some(5.0),
        max: Some(1.0),
        step: None,
    });
    assert!(fit(&reversed, &Data::Float(3.0)).is_some());
}

#[test]
fn inputs_start_from_the_readback_the_default_or_the_lowest_value() {
    let mut speed = param(ParamKind::Int {
        min: Some(100),
        max: Some(3000),
    });
    assert_eq!(initial(&speed, None), Data::Uint(100));
    speed.default = Some(Data::Uint(1500));
    assert_eq!(initial(&speed, None), Data::Uint(1500));
    assert_eq!(initial(&speed, Some(Data::Float(1432.4))), Data::Uint(1432));
    // a readback that does not fit is ignored
    assert_eq!(initial(&speed, Some(Data::from("fast"))), Data::Uint(1500));
    let level = param(ParamKind::Enum(vec!["info".to_owned(), "warn".to_owned()]));
    assert_eq!(initial(&level, None), Data::from("info"));
    let ratio = param(ParamKind::Float {
        min: Some(-1.0),
        max: None,
        step: None,
    });
    assert_eq!(initial(&ratio, None), Data::Float(0.0));
}

#[test]
fn steps_show_their_decimals() {
    assert_eq!(decimals(1.0), 0);
    assert_eq!(decimals(0.5), 1);
    assert_eq!(decimals(0.25), 2);
}

struct State {
    view: Actions,
    model: Model,
    invoker: Invoker,
    time: crate::app::time::TimeRange,
}

fn action(path: &str, params: Vec<Param>) -> ActionDescriptor {
    ActionDescriptor {
        path: path.to_owned(),
        description: String::new(),
        params,
        readback: None,
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }
}

fn named(name: &str, kind: ParamKind) -> Param {
    Param {
        name: name.to_owned(),
        ..param(kind)
    }
}

/// A harness showing a view of `actions`, announced by the connection without a
/// name.
fn harness(actions: Vec<ActionDescriptor>) -> egui_kittest::Harness<'static, State> {
    stepped_harness(actions, 1.0 / 4.0)
}

/// A harness like [`harness`] whose frames are `step_dt` seconds apart.
fn stepped_harness(
    actions: Vec<ActionDescriptor>,
    step_dt: f32,
) -> egui_kittest::Harness<'static, State> {
    let view = Actions {
        shown: actions.iter().map(|action| action.path.clone()).collect(),
        ..Actions::default()
    };
    let mut model = Model::default();
    model.actions.set("", actions);
    let mut invoker = Invoker::default();
    invoker.connected.insert(String::new());
    invoker.recording = true;
    let state = State {
        view,
        model,
        invoker,
        time: crate::app::time::TimeRange::default(),
    };
    let mut harness = egui_kittest::Harness::builder()
        .with_step_dt(step_dt)
        .build_ui_state(
            |ui, state: &mut State| {
                let mut context = ViewContext {
                    model: &state.model,
                    time: &mut state.time,
                    invoker: &mut state.invoker,
                    formats: &super::super::NO_FORMATS,
                    titled: false,
                    plots: &super::super::NO_PLOTS,
                };
                state.view.view(ui, &mut context);
            },
            state,
        );
    harness.run();
    harness
}

/// The paths and arguments of the invocations sent since the last call.
fn sent(harness: &mut egui_kittest::Harness<'_, State>) -> Vec<(String, Data)> {
    harness
        .state_mut()
        .invoker
        .take_outbox()
        .into_iter()
        .map(|(_, proto::Request::Invoke(invocation))| (invocation.path, invocation.args))
        .collect()
}

/// The connections and paths of the invocations sent since the last call.
fn sent_to(harness: &mut egui_kittest::Harness<'_, State>) -> Vec<(String, String)> {
    harness
        .state_mut()
        .invoker
        .take_outbox()
        .into_iter()
        .map(|(source, proto::Request::Invoke(invocation))| (source, invocation.path))
        .collect()
}

#[test]
fn buttons_and_toggles_send_right_away() {
    let mut harness = harness(vec![
        action("reset", vec![]),
        action("enable", vec![named("on", ParamKind::Bool)]),
    ]);
    harness.get_by_label(&format!("{PLAY} Run")).click();
    harness.run_steps(2);
    assert_eq!(
        sent(&mut harness),
        [("reset".to_owned(), Data::Record(vec![]))]
    );
    harness.get_by_role(egui::accesskit::Role::CheckBox).click();
    harness.run_steps(2);
    assert_eq!(
        sent(&mut harness),
        [(
            "enable".to_owned(),
            Data::Record(vec![("on".to_owned(), Data::Bool(true))])
        )]
    );
}

#[test]
fn several_parameters_are_sent_with_the_send_button() {
    let kinds = ParamKind::Enum(vec!["slow".to_owned(), "fast".to_owned()]);
    let mut harness = harness(vec![action(
        "move",
        vec![named("mode", kinds), named("far", ParamKind::Bool)],
    )]);
    harness.get_by_label("fast").click();
    harness.get_by_role(egui::accesskit::Role::CheckBox).click();
    harness.run_steps(2);
    assert!(sent(&mut harness).is_empty());
    harness.get_by_label("Send").click();
    harness.run_steps(2);
    let args = Data::Record(vec![
        ("mode".to_owned(), Data::from("fast")),
        ("far".to_owned(), Data::Bool(true)),
    ]);
    assert_eq!(sent(&mut harness), [("move".to_owned(), args)]);
}

#[test]
fn several_parameters_chosen_to_be_sent_on_change_are_sent_at_most_every_interval() {
    let kinds = ParamKind::Enum(vec!["slow".to_owned(), "fast".to_owned()]);
    let mut harness = stepped_harness(
        vec![action(
            "move",
            vec![named("mode", kinds), named("far", ParamKind::Bool)],
        )],
        0.01,
    );
    harness.state_mut().view.on_change.insert("move".to_owned());
    harness.run();
    assert!(harness.query_by_label("Send").is_none());
    let args = |mode: &str, far: bool| {
        Data::Record(vec![
            ("mode".to_owned(), Data::from(mode)),
            ("far".to_owned(), Data::Bool(far)),
        ])
    };

    harness.get_by_label("fast").click();
    harness.run_steps(2);
    assert_eq!(
        sent(&mut harness),
        [("move".to_owned(), args("fast", false))]
    );
    // a change right after is held back until the interval has passed
    harness.get_by_role(egui::accesskit::Role::CheckBox).click();
    harness.run_steps(2);
    assert!(sent(&mut harness).is_empty());
    let now = harness.ctx.input(|input| input.time);
    harness.state_mut().invoker.update(now + DRAG_INTERVAL);
    assert_eq!(
        sent(&mut harness),
        [("move".to_owned(), args("fast", true))]
    );
}

#[test]
fn confirmed_actions_keep_their_send_button() {
    let confirmed = ActionDescriptor {
        confirm: true,
        ..action(
            "move",
            vec![
                named("far", ParamKind::Bool),
                named("slow", ParamKind::Bool),
            ],
        )
    };
    assert!(!can_send_on_change(&confirmed));
    let mut harness = harness(vec![confirmed]);
    harness.state_mut().view.on_change.insert("move".to_owned());
    harness.run();
    harness.get_by_label("Send…");
}

fn reboot() -> ActionDescriptor {
    ActionDescriptor {
        confirm: true,
        description: "Restarts the device".to_owned(),
        ..action("reboot", vec![])
    }
}

#[test]
fn confirmed_actions_are_selected_then_carried_out_in_their_row() {
    let mut harness = harness(vec![reboot()]);
    harness.get_by_label(&format!("{PLAY} Run…")).click();
    harness.run_steps(2);
    assert!(sent(&mut harness).is_empty());
    // the description says what it does
    harness.get_by_label("Restarts the device");
    harness.get_by_label("Run reboot").click();
    harness.run_steps(2);
    assert_eq!(sent(&mut harness).len(), 1);
    assert!(harness.query_by_label("Run reboot").is_none());
    assert!(harness.query_by_label("Restarts the device").is_none());
}

#[test]
fn selected_actions_are_cancelled_or_lapse() {
    let mut harness = harness(vec![reboot()]);
    let run = format!("{PLAY} Run…");
    harness.get_by_label(&run).click();
    harness.run_steps(2);
    harness.get_by_label("Cancel").click();
    harness.run_steps(2);
    assert!(harness.query_by_label("Run reboot").is_none());

    harness.get_by_label(&run).click();
    harness.run_steps(2);
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    assert!(harness.query_by_label("Run reboot").is_none());

    harness.get_by_label(&run).click();
    harness.run_steps(2);
    // frames of a quarter of a second
    harness.run_steps((ARM_SECONDS * 4.0) as usize - 4);
    harness.get_by_label("Run reboot");
    harness.run_steps(5);
    assert!(harness.query_by_label("Run reboot").is_none());
    assert!(sent(&mut harness).is_empty());
}

#[test]
fn selected_actions_lapse_once_disabled() {
    let mut harness = harness(vec![reboot()]);
    harness.get_by_label(&format!("{PLAY} Run…")).click();
    harness.run_steps(2);
    // the target disables the action while it is selected
    let disabled = ActionDescriptor {
        enabled: false,
        ..reboot()
    };
    harness.state_mut().model.actions.set("", vec![disabled]);
    harness.run_steps(2);
    assert!(harness.query_by_label("Run reboot").is_none());
    // and enables it again: it is not selected any more
    harness.state_mut().model.actions.set("", vec![reboot()]);
    harness.run_steps(2);
    assert!(harness.query_by_label("Run reboot").is_none());
    assert!(sent(&mut harness).is_empty());
}

#[test]
fn confirmed_forms_send_their_values_and_lapse_when_changed() {
    let move_to = ActionDescriptor {
        confirm: true,
        ..action("move", vec![named("far", ParamKind::Bool)])
    };
    let mut harness = harness(vec![move_to]);
    harness.get_by_label("Send…").click();
    harness.run_steps(2);
    harness.get_by_label("Send move");
    harness.get_by_role(egui::accesskit::Role::CheckBox).click();
    harness.run_steps(2);
    assert!(harness.query_by_label("Send move").is_none());
    harness.get_by_label("Send…").click();
    harness.run_steps(2);
    harness.get_by_label("Send move").click();
    harness.run_steps(2);
    let args = Data::Record(vec![("far".to_owned(), Data::Bool(true))]);
    assert_eq!(sent(&mut harness), [("move".to_owned(), args)]);
}

#[test]
fn the_reason_of_a_disabled_action_shows_below_it_while_connected() {
    let calibrating = ActionDescriptor {
        enabled: false,
        disabled_reason: Some("calibrating".to_owned()),
        ..action("calibrate", vec![])
    };
    let mut harness = harness(vec![calibrating.clone()]);
    harness.get_by_label("calibrating");
    // not while disconnected, when all actions are disabled
    harness.state_mut().invoker.connected.clear();
    harness.run_steps(2);
    assert!(harness.query_by_label("calibrating").is_none());
    // nor once enabled again, if the outpost leaves the reason
    harness.state_mut().invoker.connected.insert(String::new());
    let enabled = ActionDescriptor {
        enabled: true,
        ..calibrating
    };
    harness.state_mut().model.actions.set("", vec![enabled]);
    harness.run_steps(2);
    assert!(harness.query_by_label("calibrating").is_none());
}

#[test]
fn actions_are_carried_out_only_while_recording() {
    let mut harness = harness(vec![action("reset", vec![])]);
    let run = format!("{PLAY} Run");
    assert!(harness.query_by_label(START_RECORDING).is_none());
    harness.state_mut().invoker.recording = false;
    harness.run_steps(2);
    harness.get_by_label(START_RECORDING);
    assert!(harness.get_by_label(&run).accesskit_node().is_disabled());
    harness.get_by_label(&run).click();
    harness.run_steps(2);
    assert!(sent(&mut harness).is_empty());
    // while disconnected that is the reason, not recording
    harness.state_mut().invoker.connected.clear();
    harness.run_steps(2);
    assert!(harness.query_by_label(START_RECORDING).is_none());
    harness.state_mut().invoker.connected.insert(String::new());
    harness.state_mut().invoker.recording = true;
    harness.run_steps(2);
    harness.get_by_label(&run).click();
    harness.run_steps(2);
    assert_eq!(sent(&mut harness).len(), 1);
}

#[test]
fn buttons_are_disabled_while_disconnected_or_disabled() {
    let disabled = ActionDescriptor {
        enabled: false,
        ..action("off", vec![])
    };
    let mut harness = harness(vec![action("reset", vec![]), disabled]);
    let run = format!("{PLAY} Run");
    // the number of disabled and of enabled buttons
    let buttons = |harness: &egui_kittest::Harness<'_, State>| {
        let count = |disabled: bool| {
            harness
                .query_all_by(|node| {
                    node.label().as_deref() == Some(run.as_str()) && node.is_disabled() == disabled
                })
                .count()
        };
        (count(true), count(false))
    };
    harness.state_mut().invoker.connected.clear();
    harness.run_steps(2);
    assert_eq!(buttons(&harness), (2, 0));
    harness.state_mut().invoker.connected.insert(String::new());
    harness.run_steps(2);
    assert_eq!(buttons(&harness), (1, 1));
}

#[test]
fn chosen_actions_show_also_when_not_announced() {
    let mut harness = harness(vec![action("a", vec![]), action("b", vec![])]);
    harness.state_mut().view.shown = ["b".to_owned(), "gone".to_owned()].into();
    harness.run_steps(2);
    assert!(harness.query_by_label("a").is_none());
    harness.get_by_label("b");
    harness.get_by_label("not announced by any outpost");
    // an action the chosen outpost does not announce
    harness.state_mut().view.outpost = Some("robby".to_owned());
    harness.run_steps(2);
    assert_eq!(
        harness.get_all_by_label("not announced by robby").count(),
        2
    );
}

#[test]
fn only_the_actions_checked_in_the_tree_show() {
    let mut harness = harness(vec![action("a", vec![])]);
    harness.state_mut().view.shown = BTreeSet::new();
    harness.run_steps(2);
    harness.get_by_label("Check the actions to show in the tree on the left");
}

#[test]
fn new_views_show_no_actions_until_checked() {
    use crate::app::view::{View, ViewType};

    let mut view = View::new(ViewType::Actions, "actions".to_owned());
    let shown = view.selection_mut().unwrap().paths;
    assert!(shown.is_empty());
    shown.insert("a".to_owned());
    match &view.kind {
        super::super::ViewKind::Actions(actions) => {
            assert_eq!(actions.shown, ["a".to_owned()].into());
        }
        _ => unreachable!(),
    }
}

#[test]
fn actions_are_sent_to_the_chosen_outpost() {
    let mut harness = harness(vec![]);
    let state = harness.state_mut();
    state.model.actions.set("a", vec![action("reset", vec![])]);
    state.model.actions.set("b", vec![action("reset", vec![])]);
    state.view.shown = ["reset".to_owned()].into();
    state.invoker.connected = ["a".to_owned(), "b".to_owned()].into();
    harness.run_steps(2);
    let run = format!("{PLAY} Run");
    // by default to the first outpost announcing it
    harness.get_by_label(&run).click();
    harness.run_steps(2);
    assert_eq!(
        sent_to(&mut harness),
        [("a".to_owned(), "reset".to_owned())]
    );

    harness.state_mut().view.outpost = Some("b".to_owned());
    harness.run_steps(2);
    harness.get_by_label(&run).click();
    harness.run_steps(2);
    assert_eq!(
        sent_to(&mut harness),
        [("b".to_owned(), "reset".to_owned())]
    );

    // disabled while the chosen outpost is not connected, also if another is
    harness.state_mut().invoker.connected.remove("b");
    harness.run_steps(2);
    assert!(harness.get_by_label(&run).accesskit_node().is_disabled());
}

#[test]
fn by_default_actions_are_sent_to_an_open_connection() {
    let mut harness = harness(vec![]);
    let state = harness.state_mut();
    // the outpost connected before, whose actions are still known, and the one now
    state
        .model
        .actions
        .set("old", vec![action("reset", vec![])]);
    state
        .model
        .actions
        .set("new", vec![action("reset", vec![])]);
    state.view.shown = ["reset".to_owned()].into();
    state.invoker.connected = ["new".to_owned()].into();
    harness.run_steps(2);
    harness.get_by_label(&format!("{PLAY} Run")).click();
    harness.run_steps(2);
    assert_eq!(
        sent_to(&mut harness),
        [("new".to_owned(), "reset".to_owned())]
    );
    // with none open, the first announcing it shows, disabled
    harness.state_mut().invoker.connected.clear();
    harness.run_steps(2);
    let run = format!("{PLAY} Run");
    assert!(harness.get_by_label(&run).accesskit_node().is_disabled());
}

#[test]
fn the_announced_readback_shows_and_starts_the_inputs_unless_off() {
    let enable = ActionDescriptor {
        readback: Some("on".to_owned()),
        ..action("enable", vec![named("on", ParamKind::Bool)])
    };
    let mut harness = harness(vec![enable]);
    receive(&mut harness, "on", 0, Data::Bool(true));
    let checked = |harness: &egui_kittest::Harness<'_, State>| {
        let checkbox = harness.get_by_role(egui::accesskit::Role::CheckBox);
        checkbox.accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
    };
    harness.run_steps(2);
    assert!(checked(&harness));
    // the checkbox shows the readback, the text only while they differ
    assert!(harness.query_by_label_contains("reads").is_none());
    harness.get_by_role(egui::accesskit::Role::CheckBox).click();
    harness.run_steps(2);
    assert!(!checked(&harness));
    harness.get_by_label("reads on");

    // the readback turned off
    harness.state_mut().invoker.inputs.clear();
    harness
        .state_mut()
        .view
        .readbacks_off
        .insert("enable".to_owned());
    harness.run_steps(2);
    assert!(harness.query_by_label_contains("reads").is_none());
    assert!(!checked(&harness));
}

/// A harness showing the properties of an actions view showing "speed", which the
/// connections "a" and "b" announce, "a" with the readback "rpm", with no
/// connection open.
fn properties_harness() -> egui_kittest::Harness<'static, (Actions, Model)> {
    let mut model = Model::default();
    let speed = |readback: Option<&str>| ActionDescriptor {
        readback: readback.map(str::to_owned),
        ..action("speed", vec![])
    };
    model.actions.set("a", vec![speed(Some("rpm"))]);
    model.actions.set("b", vec![speed(None)]);
    let view = Actions {
        shown: ["speed".to_owned()].into(),
        ..Actions::default()
    };
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(300.0, 600.0))
        .build_ui_state(
            |ui, (view, model): &mut (Actions, Model)| {
                let connected = HashSet::new();
                let context = PropertiesContext {
                    model,
                    connected: &connected,
                    plots: &super::super::NO_PLOTS,
                };
                view.properties_ui(ui, &context);
            },
            (view, model),
        );
    harness.run();
    harness
}

/// Opens the dropdown showing `selected` and clicks its option `option`.
fn choose(harness: &mut egui_kittest::Harness<'_, (Actions, Model)>, selected: &str, option: &str) {
    harness
        .get_by(|node| {
            node.role() == egui::accesskit::Role::ComboBox
                && node.value().as_deref() == Some(selected)
        })
        .click();
    harness.run();
    harness
        .get_all_by_label(option)
        .last()
        .expect("the option is listed")
        .click();
    harness.run();
}

/// The checkbox of the readback in the properties of `harness`.
fn readback_checkbox<'a>(
    harness: &'a egui_kittest::Harness<'_, (Actions, Model)>,
) -> egui_kittest::Node<'a> {
    harness.get_by_role(egui::accesskit::Role::CheckBox)
}

#[test]
fn the_outpost_of_the_actions_is_chosen_among_those_announcing_actions() {
    let mut harness = properties_harness();
    choose(&mut harness, "First announcing", "b");
    assert_eq!(harness.state().0.outpost.as_deref(), Some("b"));
    // the readback follows the outpost, which announces none
    let checkbox = readback_checkbox(&harness);
    assert!(checkbox.accesskit_node().is_disabled());
    assert_eq!(
        checkbox.accesskit_node().label().as_deref(),
        Some("none announced")
    );

    // a chosen outpost that is gone is still listed
    harness.state_mut().0.outpost = Some("gone".to_owned());
    harness.run();
    choose(&mut harness, "gone", "First announcing");
    assert_eq!(harness.state().0.outpost, None);
}

#[test]
fn the_announced_readback_is_turned_off_and_on() {
    let mut harness = properties_harness();
    let toggled = |harness: &egui_kittest::Harness<'_, (Actions, Model)>| {
        readback_checkbox(harness).accesskit_node().toggled()
            == Some(egui::accesskit::Toggled::True)
    };
    assert!(toggled(&harness));
    readback_checkbox(&harness).click();
    harness.run();
    assert_eq!(harness.state().0.readbacks_off, ["speed".to_owned()].into());
    assert!(!toggled(&harness));
    readback_checkbox(&harness).click();
    harness.run();
    assert!(harness.state().0.readbacks_off.is_empty());
}

#[test]
fn actions_with_several_parameters_are_switched_to_be_sent_on_change() {
    let mut harness = properties_harness();
    assert!(harness.query_by_label("On change").is_none());
    let point = action(
        "speed",
        vec![named("x", ParamKind::Bool), named("y", ParamKind::Bool)],
    );
    harness.state_mut().1.actions.set("a", vec![point]);
    harness.run();
    harness.get_by_label("On change").click();
    harness.run();
    assert_eq!(harness.state().0.on_change, ["speed".to_owned()].into());
    harness.get_by_label("Send button").click();
    harness.run();
    assert!(harness.state().0.on_change.is_empty());
}

#[test]
fn layouts_of_older_versions_load() {
    let all: Actions = serde_json::from_str(r#"{"shown": null, "fields": []}"#).unwrap();
    assert!(all.shown.is_empty() && all.outpost.is_none());
    let chosen: Actions = serde_json::from_str(
        r#"{"shown": ["a"], "outposts": {"a": "robby"}, "readbacks": {"a": null}}"#,
    )
    .unwrap();
    assert_eq!(chosen.shown, ["a".to_owned()].into());
    assert!(chosen.outpost.is_none() && chosen.readbacks_off.is_empty());

    let view = Actions {
        outpost: Some("robby".to_owned()),
        readbacks_off: ["a".to_owned()].into(),
        ..Actions::default()
    };
    let json = serde_json::to_string(&view).unwrap();
    let loaded: Actions = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.outpost, view.outpost);
    assert_eq!(loaded.readbacks_off, view.readbacks_off);
}

#[test]
fn clicking_a_slider_sends_its_new_value_once() {
    let percent = ParamKind::Int {
        min: Some(0),
        max: Some(100),
    };
    let mut harness = harness(vec![action("level", vec![named("p", percent)])]);
    harness.get_by_role(egui::accesskit::Role::Slider).click();
    harness.run_steps(3);
    let sent = sent(&mut harness);
    assert_eq!(sent.len(), 1);
    assert_ne!(
        sent[0].1,
        Data::Record(vec![("p".to_owned(), Data::Uint(0))])
    );
}

/// Receives the value at `path` from the connection without a name, while not
/// recording.
fn receive(
    harness: &mut egui_kittest::Harness<'_, State>,
    path: &str,
    timestamp: u64,
    value: Data,
) {
    let message = proto::Message::Sample(proto::Sample {
        timestamp,
        path: path.to_owned(),
        value,
    });
    let frame = serde_cbor_2::to_vec(&message).unwrap();
    harness.state_mut().model.receive_unrecorded("", &frame);
}

/// Types `text` into the only text field and enters it.
fn enter_text(harness: &mut egui_kittest::Harness<'_, State>, text: &str) {
    let field = || egui::accesskit::Role::TextInput;
    harness.get_by_role(field()).focus();
    harness.run_steps(2);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.get_by_role(field()).type_text(text);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
}

/// Answers the invocation `id` sent to the connection without a name.
fn answer(harness: &mut egui_kittest::Harness<'_, State>, id: u32, result: Result<(), &str>) {
    let result = proto::ActionResult {
        id,
        result: result.map_err(str::to_owned),
    };
    let state = harness.state_mut();
    state.invoker.receive("", &result, 0.0, &state.model);
    harness.run_steps(2);
}

fn field_text(harness: &egui_kittest::Harness<'_, State>) -> String {
    let field = harness.get_by_role(egui::accesskit::Role::TextInput);
    field.value().unwrap_or_default()
}

#[test]
fn rejected_values_go_back_to_the_accepted_one_with_the_reason() {
    let register = ParamKind::Int {
        min: None,
        max: None,
    };
    let mut harness = harness(vec![action("poke", vec![named("v", register)])]);
    enter_text(&mut harness, "5");
    answer(&mut harness, 0, Ok(()));
    enter_text(&mut harness, "7");
    assert_eq!(sent(&mut harness).len(), 2);
    answer(&mut harness, 1, Err("too big"));
    assert_eq!(field_text(&harness), "5");
    harness.get_by_label("7 rejected: too big");
    // the reason stays until the next invocation
    enter_text(&mut harness, "6");
    assert!(harness.query_by_label("7 rejected: too big").is_none());
}

#[test]
fn a_dragged_slider_goes_back_only_once_let_go() {
    let percent = ParamKind::Int {
        min: Some(0),
        max: Some(100),
    };
    let mut harness = harness(vec![action("level", vec![named("p", percent)])]);
    let input = |harness: &egui_kittest::Harness<'_, State>| {
        let key = (ActionId::new("", "level"), "p".to_owned());
        harness.state().invoker.inputs[&key].clone()
    };
    let rect = harness.get_by_role(egui::accesskit::Role::Slider).rect();
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    harness.event(egui::Event::PointerMoved(rect.left_center()));
    harness.event(press(rect.left_center(), true));
    harness.run_steps(1);
    harness.event(egui::Event::PointerMoved(rect.center()));
    harness.run_steps(2);
    let dragged = input(&harness);
    assert_ne!(dragged, Data::Uint(0));
    assert_eq!(sent(&mut harness).len(), 1);
    // rejected while still held: the slider stays and says nothing yet
    answer(&mut harness, 0, Err("too high"));
    assert_eq!(input(&harness), dragged);
    assert!(harness.query_by_label_contains("rejected").is_none());
    harness.event(press(rect.center(), false));
    harness.run_steps(3);
    assert_eq!(input(&harness), Data::Uint(0));
    assert!(sent(&mut harness).is_empty());
    harness.get_by_label_contains("rejected: too high");
}

#[test]
fn rejected_values_go_back_to_what_showed_before_and_not_the_readback() {
    let register = ParamKind::Int {
        min: None,
        max: None,
    };
    let poke = ActionDescriptor {
        readback: Some("rb".to_owned()),
        ..action("poke", vec![named("v", register)])
    };
    let mut harness = harness(vec![poke]);
    let sample = |harness: &mut egui_kittest::Harness<'_, State>, timestamp, value| {
        receive(harness, "rb", timestamp, Data::Uint(value));
        harness.run_steps(2);
    };
    sample(&mut harness, 0, 10);
    // the input starts from the readback
    harness.state_mut().invoker.inputs.clear();
    harness.run_steps(2);
    assert_eq!(field_text(&harness), "10");
    enter_text(&mut harness, "50");
    // the readback moves on, e.g. a speed still settling
    sample(&mut harness, 1, 4);
    answer(&mut harness, 0, Err("no"));
    assert_eq!(field_text(&harness), "10");
}

#[test]
fn rejected_text_is_kept_to_correct_it() {
    let mut harness = harness(vec![action("say", vec![named("text", ParamKind::String)])]);
    enter_text(&mut harness, "helo");
    answer(&mut harness, 0, Err("misspelt"));
    assert_eq!(field_text(&harness), "helo");
    harness.get_by_label("helo rejected: misspelt");
}

#[test]
fn rejected_values_without_an_accepted_one_go_back_to_the_default() {
    let mut ratio = named(
        "r",
        ParamKind::Int {
            min: None,
            max: None,
        },
    );
    ratio.default = Some(Data::Uint(3));
    let mut harness = harness(vec![action("ratio", vec![ratio])]);
    enter_text(&mut harness, "9");
    answer(&mut harness, 0, Err("no"));
    assert_eq!(field_text(&harness), "3");
}

#[test]
fn forms_keep_their_inputs_when_rejected() {
    let mut harness = harness(vec![action(
        "message",
        vec![
            named(
                "level",
                ParamKind::Enum(vec!["info".to_owned(), "warn".to_owned()]),
            ),
            named("text", ParamKind::String),
        ],
    )]);
    enter_text(&mut harness, "pump stalled");
    harness.get_by_label("warn").click();
    harness.get_by_label("Send").click();
    harness.run_steps(2);
    answer(&mut harness, 0, Err("busy"));
    assert_eq!(field_text(&harness), "pump stalled");
    harness.get_by_label("Rejected: busy");
}

#[test]
fn changed_inputs_of_a_form_are_marked_until_sent() {
    let mut harness = harness(vec![action(
        "message",
        vec![
            named(
                "level",
                ParamKind::Enum(vec!["info".to_owned(), "warn".to_owned()]),
            ),
            named("far", ParamKind::Bool),
        ],
    )]);
    assert!(harness.query_by_label("Changed").is_none());
    harness.get_by_label("warn").click();
    harness.run_steps(2);
    assert_eq!(harness.query_all_by_label("Changed").count(), 1);
    harness.get_by_role(egui::accesskit::Role::CheckBox).click();
    harness.run_steps(2);
    assert_eq!(harness.query_all_by_label("Changed").count(), 2);
    harness.get_by_label("Send").click();
    harness.run_steps(2);
    assert_eq!(sent(&mut harness).len(), 1);
    assert!(harness.query_by_label("Changed").is_none());
    // changing back to what was sent is no change
    harness.get_by_label("info").click();
    harness.run_steps(2);
    harness.get_by_label("warn").click();
    harness.run_steps(2);
    assert!(harness.query_by_label("Changed").is_none());
}

#[test]
fn forms_sent_on_change_and_inputs_sent_at_once_are_not_marked() {
    let mut harness = harness(vec![
        action("enable", vec![named("on", ParamKind::Bool)]),
        action(
            "move",
            vec![
                named(
                    "mode",
                    ParamKind::Enum(vec!["slow".to_owned(), "fast".to_owned()]),
                ),
                named("far", ParamKind::Bool),
            ],
        ),
    ]);
    harness.state_mut().view.on_change.insert("move".to_owned());
    harness.run();
    harness.get_by_label("fast").click();
    harness.run_steps(2);
    harness
        .get_all_by_role(egui::accesskit::Role::CheckBox)
        .next()
        .unwrap()
        .click();
    harness.run_steps(2);
    assert!(harness.query_by_label("Changed").is_none());
}

#[test]
fn text_fields_hint_at_enter_until_sent() {
    let mut harness = harness(vec![action("say", vec![named("text", ParamKind::String)])]);
    let field = || egui::accesskit::Role::TextInput;
    harness.get_by_role(field()).focus();
    harness.run_steps(2);
    harness.get_by_role(field()).type_text("hello");
    harness.run_steps(2);
    harness.get_by_label("Enter to send");
    harness.get_by_label("Changed");
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert_eq!(sent(&mut harness).len(), 1);
    assert!(harness.query_by_label("Enter to send").is_none());
    assert!(harness.query_by_label("Changed").is_none());
}

#[test]
fn typed_integers_are_marked_until_entered() {
    let register = || ParamKind::Int {
        min: None,
        max: None,
    };
    let mut harness = harness(vec![action("poke", vec![named("v", register())])]);
    let field = || egui::accesskit::Role::TextInput;
    harness.get_by_role(field()).focus();
    harness.run_steps(2);
    // focusing alone changes nothing
    assert!(harness.query_by_label("Enter to send").is_none());
    harness.get_by_role(field()).type_text("12");
    harness.run_steps(2);
    harness.get_by_label("Enter to send");
    harness.get_by_label("Changed");
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert_eq!(sent(&mut harness).len(), 1);
    assert!(harness.query_by_label("Enter to send").is_none());
    assert!(harness.query_by_label("Changed").is_none());

    // in a form with a Send button the parameter gets the dot
    let mut harness = harness_form(vec![named("v", register()), named("on", ParamKind::Bool)]);
    harness.get_by_role(field()).focus();
    harness.run_steps(2);
    harness.get_by_role(field()).type_text("7");
    harness.run_steps(2);
    harness.get_by_label("Changed");
    assert!(sent(&mut harness).is_empty());
}

fn harness_form(params: Vec<Param>) -> egui_kittest::Harness<'static, State> {
    harness(vec![action("form", params)])
}

#[test]
fn pending_invocations_count_the_seconds_after_the_first() {
    let mut harness = harness(vec![action("reset", vec![])]);
    harness.get_by_label(&format!("{PLAY} Run")).click();
    harness.run_steps(2);
    assert!(harness.query_by_label("1 s").is_none());
    // frames of a quarter of a second
    harness.run_steps(4);
    harness.get_by_label("1 s");
}

#[test]
fn unanswered_invocations_say_how_long_they_wait() {
    let mut harness = harness(vec![action("reset", vec![])]);
    harness.get_by_label(&format!("{PLAY} Run")).click();
    harness.run_steps(2);
    assert_eq!(sent(&mut harness).len(), 1);
    // six seconds in frames of a quarter
    for _ in 0..24 {
        let now = harness.ctx.input(|input| input.time);
        harness.state_mut().invoker.update(now);
        harness.run_steps(1);
    }
    harness.get_by_label_contains("s, a late answer still counts");
    // a late answer still counts
    answer(&mut harness, 0, Err("busy"));
    harness.get_by_label("Rejected: busy");
}

#[test]
fn typed_integers_are_sent_exactly_on_enter() {
    let register = ParamKind::Int {
        min: None,
        max: None,
    };
    let mut harness = harness(vec![action("poke", vec![named("v", register)])]);
    let field = || egui::accesskit::Role::TextInput;
    harness.get_by_role(field()).focus();
    harness.run_steps(2);
    harness.get_by_role(field()).type_text("9007199254740993");
    harness.run_steps(2);
    assert!(sent(&mut harness).is_empty());
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    let args = Data::Record(vec![("v".to_owned(), Data::Uint((1 << 53) + 1))]);
    assert_eq!(sent(&mut harness), [("poke".to_owned(), args)]);

    // text that is no integer is dropped
    harness.get_by_role(field()).focus();
    harness.run_steps(2);
    harness.get_by_role(field()).type_text("x");
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert!(sent(&mut harness).is_empty());
}

#[test]
fn typed_numbers_are_sent_on_enter_only() {
    let ratio = ParamKind::Float {
        min: None,
        max: None,
        step: None,
    };
    let mut harness = harness(vec![action("ratio", vec![named("r", ratio)])]);
    harness
        .get_by_role(egui::accesskit::Role::SpinButton)
        .click();
    harness.run_steps(2);
    // neither clicking the field nor typing sends
    harness
        .get_by_role(egui::accesskit::Role::SpinButton)
        .type_text("125");
    harness.run_steps(2);
    assert!(sent(&mut harness).is_empty());
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    let value = |args: &Data| match args {
        Data::Record(fields) => fields[0].1.clone(),
        _ => Data::Null,
    };
    let sent: Vec<Data> = sent(&mut harness)
        .iter()
        .map(|(_, args)| value(args))
        .collect();
    assert_eq!(sent, [Data::Float(125.0)]);
}
