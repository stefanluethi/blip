//! A form of the actions the target announced, to invoke them.
//!
//! An action without parameters is a button. An action with one parameter is sent as
//! soon as its input changes: a toggle, a choice, a slider while it is dragged (at most
//! every [`DRAG_INTERVAL`](crate::app::model::actions::DRAG_INTERVAL)), a number or text field on Enter. An action with several
//! parameters, or one that asks for confirmation, is sent with its Send button.

use super::{ViewContext, Viewable, properties_section, short_names};
use crate::app::model::Model;
use crate::app::model::actions::{Args, Invoker, Outcome};
use crate::app::model::numbers::{DisplayFormat, Number};
use crate::app::model::traces::Samples;
use egui_phosphor::regular::{CHECK_CIRCLE, CLOCK_COUNTDOWN, PLAY, WARNING_CIRCLE};
use proto::{ActionDescriptor, Data, Param, ParamKind};
use std::collections::BTreeSet;

/// Choices of at most this many options are buttons side by side, longer ones a
/// dropdown.
const MAX_BUTTON_OPTIONS: usize = 4;
/// The width of the inputs in points.
const INPUT_WIDTH: f32 = 180.0;

/// The actions shown and how their inputs look.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct Actions {
    /// The paths of the shown actions, all announced actions if `None`.
    pub shown: Option<BTreeSet<String>>,
    /// The numeric parameters with bounds entered in a field instead of a slider, by
    /// the path of the action and the parameter name joined with ":".
    pub fields: BTreeSet<String>,
}

/// How a change of an input sends its action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Change {
    None,
    /// The input is dragged, send at most every [`DRAG_INTERVAL`](crate::app::model::actions::DRAG_INTERVAL).
    Dragging,
    /// The input got its new value, send it now.
    Done,
}

/// Why actions cannot be invoked now, if they cannot.
fn disabled_reason(invoker: &Invoker, action: &ActionDescriptor) -> Option<&'static str> {
    if !invoker.connected {
        Some("Not connected to a target")
    } else if !action.enabled {
        Some("The target cannot carry out the action now")
    } else {
        None
    }
}

/// Whether the action is sent with a Send button rather than when its input changes.
fn has_send_button(action: &ActionDescriptor) -> bool {
    action.params.len() > 1 || (action.confirm && !action.params.is_empty())
}

/// The latest value of the trace at `path`, if it is a number, bool or string.
fn latest_value(model: &Model, path: &str) -> Option<Data> {
    let trace = model.traces.get(path)?;
    let last = trace.timestamps.len().checked_sub(1)?;
    match &trace.samples {
        Samples::Numbers(numbers) => match numbers.get(last) {
            Number::Null => None,
            Number::Int(value) => Some(Data::Int(value)),
            Number::Uint(value) => Some(Data::Uint(value)),
            Number::Float(value) => Some(Data::Float(value)),
            Number::Bool(value) => Some(Data::Bool(value)),
        },
        Samples::Strings(values) => values[last].clone().map(Data::String),
        _ => None,
    }
}

/// The latest value of the trace at `path` as text with its unit.
fn latest_text(model: &Model, path: &str) -> Option<String> {
    let trace = model.traces.get(path)?;
    let last = trace.timestamps.len().checked_sub(1)?;
    let value = trace
        .samples
        .format(last, 4, Some(6), DisplayFormat::Default);
    Some(super::with_unit(value, model.traces.unit(path)))
}

fn as_f64(data: &Data) -> Option<f64> {
    match *data {
        Data::Uint(value) => Some(value as f64),
        Data::Int(value) => Some(value as f64),
        Data::Float(value) => Some(value),
        _ => None,
    }
}

/// An integer as data, unsigned unless negative as on the wire.
fn integer(value: i64) -> Data {
    u64::try_from(value).map_or(Data::Int(value), Data::Uint)
}

/// `data` as a value of `param`, or `None` if it does not fit.
fn fit(param: &Param, data: &Data) -> Option<Data> {
    match (&param.kind, data) {
        (ParamKind::Bool, Data::Bool(_)) | (ParamKind::String, Data::String(_)) => {
            Some(data.clone())
        }
        (ParamKind::Enum(options), Data::String(value)) => {
            options.contains(value).then(|| data.clone())
        }
        (ParamKind::Int { min, max }, data) => {
            let value = as_f64(data)?.round() as i64;
            Some(integer(
                value.clamp(min.unwrap_or(i64::MIN), max.unwrap_or(i64::MAX)),
            ))
        }
        (ParamKind::Float { min, max, .. }, data) => {
            let value = as_f64(data)?;
            Some(Data::Float(
                value.clamp(min.unwrap_or(f64::MIN), max.unwrap_or(f64::MAX)),
            ))
        }
        _ => None,
    }
}

/// The value an input of `param` starts with: the readback, the default of the
/// parameter, or the lowest value it takes.
fn initial(param: &Param, readback: Option<Data>) -> Data {
    if let Some(value) = readback.as_ref().and_then(|data| fit(param, data)) {
        return value;
    }
    if let Some(value) = param.default.as_ref().and_then(|data| fit(param, data)) {
        return value;
    }
    match &param.kind {
        ParamKind::Bool => Data::Bool(false),
        ParamKind::Int { min, .. } => integer(min.unwrap_or(0).max(0)),
        ParamKind::Float { min, .. } => Data::Float(min.unwrap_or(0.0).max(0.0)),
        ParamKind::Enum(options) => options
            .first()
            .map_or(Data::Null, |option| Data::String(option.clone())),
        ParamKind::String => Data::String(String::new()),
    }
}

/// Shows an input for `value` of `param`. Numbers with both bounds get a slider unless
/// `field`.
fn input_ui(ui: &mut egui::Ui, param: &Param, value: &mut Data, field: bool) -> Change {
    let unit_suffix = if param.unit.is_empty() {
        String::new()
    } else {
        format!(" {}", param.unit)
    };
    match &param.kind {
        ParamKind::Bool => {
            let mut checked = matches!(value, Data::Bool(true));
            let response = ui.checkbox(&mut checked, "");
            *value = Data::Bool(checked);
            if response.changed() {
                Change::Done
            } else {
                Change::None
            }
        }
        ParamKind::Enum(options) => {
            let mut selected = match value {
                Data::String(selected) => selected.clone(),
                _ => String::new(),
            };
            let mut changed = false;
            if options.len() <= MAX_BUTTON_OPTIONS {
                ui.horizontal(|ui| {
                    for option in options {
                        changed |= ui
                            .selectable_value(&mut selected, option.clone(), option)
                            .changed();
                    }
                });
            } else {
                egui::ComboBox::from_id_salt(ui.id().with(("choice", &param.name)))
                    .selected_text(&selected)
                    .width(INPUT_WIDTH)
                    .show_ui(ui, |ui| {
                        for option in options {
                            changed |= ui
                                .selectable_value(&mut selected, option.clone(), option)
                                .changed();
                        }
                    });
            }
            *value = Data::String(selected);
            if changed { Change::Done } else { Change::None }
        }
        ParamKind::Int { min, max } => {
            let mut number = as_f64(value).unwrap_or(0.0);
            let bounds = (min.map(|v| v as f64), max.map(|v| v as f64));
            let response = number_ui(ui, &mut number, bounds, Some(1.0), field, &unit_suffix);
            *value = integer(number.round() as i64);
            change_of(&response)
        }
        ParamKind::Float { min, max, step } => {
            let mut number = as_f64(value).unwrap_or(0.0);
            let response = number_ui(ui, &mut number, (*min, *max), *step, field, &unit_suffix);
            *value = Data::Float(number);
            change_of(&response)
        }
        ParamKind::String => {
            let mut text = match value {
                Data::String(text) => text.clone(),
                _ => String::new(),
            };
            let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(INPUT_WIDTH));
            *value = Data::String(text);
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                Change::Done
            } else {
                Change::None
            }
        }
    }
}

/// How a slider or number field changed: dragging while dragged, done when released,
/// typed in or changed with the keys.
fn change_of(response: &egui::Response) -> Change {
    if response.drag_stopped() {
        Change::Done
    } else if response.changed() && response.dragged() {
        Change::Dragging
    } else if response.changed() {
        Change::Done
    } else {
        Change::None
    }
}

fn number_ui(
    ui: &mut egui::Ui,
    number: &mut f64,
    (min, max): (Option<f64>, Option<f64>),
    step: Option<f64>,
    field: bool,
    suffix: &str,
) -> egui::Response {
    match (min, max) {
        (Some(min), Some(max)) if !field => {
            ui.spacing_mut().slider_width = INPUT_WIDTH - 60.0;
            let mut slider = egui::Slider::new(number, min..=max).suffix(suffix);
            if let Some(step) = step {
                slider = slider.step_by(step);
            }
            ui.add(slider)
        }
        _ => {
            let speed = match (min, max) {
                (Some(min), Some(max)) => ((max - min) / 200.0).max(step.unwrap_or(0.0)),
                _ => step.unwrap_or(0.1),
            };
            let mut drag = egui::DragValue::new(number)
                .speed(speed)
                .range(min.unwrap_or(f64::NEG_INFINITY)..=max.unwrap_or(f64::INFINITY))
                .suffix(suffix);
            if let Some(step) = step {
                drag = drag.fixed_decimals(decimals(step));
            }
            ui.add(drag)
        }
    }
}

/// The decimals that show multiples of `step`.
fn decimals(step: f64) -> usize {
    let mut decimals = 0;
    while decimals < 6 && (step * 10f64.powi(decimals as i32)).fract().abs() > 1e-9 {
        decimals += 1;
    }
    decimals
}

/// Shows the outcome of the latest invocation of `path`.
fn outcome_ui(ui: &mut egui::Ui, invoker: &Invoker, path: &str, now: f64) {
    match invoker.outcome(path, now) {
        None => {
            ui.label("");
        }
        Some(Outcome::Pending) => {
            ui.add(egui::Spinner::new().size(12.0))
                .on_hover_text("Waiting for the target");
        }
        Some(Outcome::Accepted(_)) => {
            ui.colored_label(ui.visuals().widgets.active.fg_stroke.color, CHECK_CIRCLE)
                .on_hover_text("Carried out");
        }
        Some(Outcome::Rejected(reason)) => {
            ui.colored_label(ui.visuals().error_fg_color, WARNING_CIRCLE)
                .on_hover_text(format!("Rejected: {reason}"));
        }
        Some(Outcome::Unanswered) => {
            ui.colored_label(ui.visuals().warn_fg_color, CLOCK_COUNTDOWN)
                .on_hover_text("The target did not answer");
        }
    }
}

/// An action asked to be confirmed before it is sent.
#[derive(Clone)]
struct Confirmation {
    path: String,
    args: Args,
}

impl Actions {
    pub fn new() -> Self {
        Self::default()
    }

    /// The actions shown, in the order the target announced them, followed by the
    /// paths of shown actions it does not announce.
    fn rows<'a>(&'a self, model: &'a Model) -> (Vec<&'a ActionDescriptor>, Vec<&'a str>) {
        let announced: Vec<&ActionDescriptor> = model
            .actions
            .iter()
            .filter(|action| {
                self.shown
                    .as_ref()
                    .is_none_or(|shown| shown.contains(&action.path))
            })
            .collect();
        let missing = self
            .shown
            .iter()
            .flatten()
            .filter(|path| model.actions.get(path).is_none());
        (announced, missing.map(String::as_str).collect())
    }

    fn uses_field(&self, path: &str, param: &str) -> bool {
        self.fields.contains(&format!("{path}:{param}"))
    }

    /// Shows the row of `action`, with a row per parameter if it has a Send button.
    fn action_ui(
        &self,
        ui: &mut egui::Ui,
        context: &mut ViewContext<'_>,
        action: &ActionDescriptor,
        name: &str,
        now: f64,
        confirm: &mut Option<Confirmation>,
    ) {
        let invoker = &mut *context.invoker;
        let disabled = disabled_reason(invoker, action);
        let readback = action
            .readback
            .as_deref()
            .and_then(|path| Some((path, latest_text(context.model, path)?)));
        let mut hover = action.path.clone();
        if !action.description.is_empty() {
            hover = format!("{}\n{hover}", action.description);
        }

        // the inputs, starting from the readback or the defaults
        let follows_readback = action.params.len() == 1
            && matches!(action.params[0].kind, ParamKind::Bool | ParamKind::Enum(_))
            && invoker.outcome(&action.path, now) != Some(&Outcome::Pending);
        let readback_value = action
            .readback
            .as_deref()
            .and_then(|path| latest_value(context.model, path));
        let mut values: Vec<Data> = action
            .params
            .iter()
            .map(|param| {
                let key = (action.path.clone(), param.name.clone());
                let single = action.params.len() == 1;
                let readback = readback_value.clone().filter(|_| single);
                if follows_readback
                    && let Some(value) = readback.as_ref().and_then(|data| fit(param, data))
                {
                    return value;
                }
                invoker
                    .inputs
                    .get(&key)
                    .and_then(|data| fit(param, data))
                    .unwrap_or_else(|| initial(param, readback))
            })
            .collect();

        let send_button = has_send_button(action);
        let mut change = Change::None;
        ui.label(name).on_hover_text(&hover);
        ui.add_enabled_ui(disabled.is_none(), |ui| {
            let input = if action.params.is_empty() {
                let button = ui.button(format!("{PLAY} Run"));
                if button.clicked() {
                    change = Change::Done;
                }
                button
            } else if !send_button {
                let field = self.uses_field(&action.path, &action.params[0].name);
                ui.horizontal(|ui| {
                    change = input_ui(ui, &action.params[0], &mut values[0], field);
                })
                .response
            } else {
                ui.label("")
            };
            if let Some(reason) = disabled {
                input.on_disabled_hover_text(reason);
            }
        });
        match readback {
            Some((path, text)) => ui.label(text).on_hover_text(format!("Readback of {path}")),
            None => ui.label(""),
        };
        outcome_ui(ui, invoker, &action.path, now);
        ui.end_row();

        if send_button {
            for (param, value) in action.params.iter().zip(&mut values) {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.weak(&param.name);
                });
                ui.add_enabled_ui(disabled.is_none(), |ui| {
                    let field = self.uses_field(&action.path, &param.name);
                    ui.horizontal(|ui| input_ui(ui, param, value, field));
                });
                ui.label("");
                ui.label("");
                ui.end_row();
            }
            ui.label("");
            ui.add_enabled_ui(disabled.is_none(), |ui| {
                if ui.button("Send").clicked() {
                    change = Change::Done;
                }
            });
            ui.label("");
            ui.label("");
            ui.end_row();
        }

        for (param, value) in action.params.iter().zip(&values) {
            invoker
                .inputs
                .insert((action.path.clone(), param.name.clone()), value.clone());
        }
        if disabled.is_some() || change == Change::None {
            return;
        }
        let args: Args = action
            .params
            .iter()
            .map(|param| param.name.clone())
            .zip(values)
            .collect();
        if action.confirm {
            *confirm = Some(Confirmation {
                path: action.path.clone(),
                args,
            });
        } else if change == Change::Dragging {
            invoker.invoke_dragging(&action.path, args, now);
        } else {
            invoker.invoke(&action.path, args, now);
        }
    }

    /// Asks to confirm the action held in `confirm`, invoking it if confirmed.
    fn confirm_ui(ui: &mut egui::Ui, invoker: &mut Invoker, id: egui::Id, now: f64) {
        let Some(confirmation) = ui.data(|data| data.get_temp::<Confirmation>(id)) else {
            return;
        };
        let mut decided = false;
        egui::Modal::new(id.with("modal")).show(ui.ctx(), |ui| {
            ui.set_max_width(320.0);
            ui.label(format!("Carry out {}?", confirmation.path));
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if ui.button("Run").clicked() {
                    invoker.invoke(&confirmation.path, confirmation.args.clone(), now);
                    decided = true;
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    decided = true;
                }
            });
        });
        if decided {
            ui.data_mut(|data| data.remove::<Confirmation>(id));
        }
    }
}

impl Viewable for Actions {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let now = ui.input(|input| input.time);
        let model = context.model;
        let (announced, missing) = self.rows(model);
        if announced.is_empty() && missing.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.weak(if model.actions.is_empty() {
                    "The target announced no actions"
                } else {
                    "Choose the actions to show in the properties"
                });
            });
            return;
        }
        let paths = announced
            .iter()
            .map(|action| action.path.as_str())
            .chain(missing.iter().copied());
        let names = short_names(paths);
        let confirm_id = ui.id().with("confirm");
        let mut confirm = None;
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::NONE.inner_margin(8.0).show(ui, |ui| {
                    egui::Grid::new(ui.id().with("actions"))
                        .num_columns(4)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            for (action, name) in announced.iter().zip(&names) {
                                self.action_ui(ui, context, action, name, now, &mut confirm);
                            }
                            for (path, name) in missing.iter().zip(&names[announced.len()..]) {
                                ui.weak(*name).on_hover_text(*path);
                                ui.weak("not announced by the target");
                                ui.end_row();
                            }
                        });
                });
            });
        if let Some(confirmation) = confirm {
            ui.data_mut(|data| data.insert_temp(confirm_id, confirmation));
        }
        Self::confirm_ui(ui, context.invoker, confirm_id, now);
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, model: &Model) {
        properties_section(ui, "Actions", |ui| {
            let mut all = self.shown.is_none();
            super::choice_row(ui, "Show", &mut all, &[(true, "All"), (false, "Chosen")]);
            if all != self.shown.is_none() {
                self.shown = (!all).then(|| model.actions.iter().map(|a| a.path.clone()).collect());
            }
        });
        if let Some(shown) = &mut self.shown {
            properties_section(ui, "Shown actions", |ui| {
                let paths: Vec<String> = model
                    .actions
                    .iter()
                    .map(|action| action.path.clone())
                    .chain(
                        shown
                            .iter()
                            .filter(|p| model.actions.get(p).is_none())
                            .cloned(),
                    )
                    .collect();
                if paths.is_empty() {
                    ui.weak("The target announced no actions");
                    ui.end_row();
                }
                for path in paths {
                    let mut checked = shown.contains(&path);
                    let announced = model.actions.get(&path).is_some();
                    let text = if announced {
                        egui::RichText::new(&path)
                    } else {
                        egui::RichText::new(&path).weak()
                    };
                    if ui.checkbox(&mut checked, text).changed() {
                        if checked {
                            shown.insert(path);
                        } else {
                            shown.remove(&path);
                        }
                    }
                    ui.end_row();
                }
            });
        }

        let bounded: Vec<(String, String)> = model
            .actions
            .iter()
            .filter(|action| {
                self.shown
                    .as_ref()
                    .is_none_or(|shown| shown.contains(&action.path))
            })
            .flat_map(|action| {
                action.params.iter().filter_map(move |param| {
                    let bounded = match param.kind {
                        ParamKind::Int { min, max } => min.is_some() && max.is_some(),
                        ParamKind::Float { min, max, .. } => min.is_some() && max.is_some(),
                        _ => false,
                    };
                    bounded.then(|| (action.path.clone(), param.name.clone()))
                })
            })
            .collect();
        if bounded.is_empty() {
            return;
        }
        properties_section(ui, "Number inputs", |ui| {
            for (path, param) in bounded {
                let key = format!("{path}:{param}");
                let mut field = self.fields.contains(&key);
                let label = if model
                    .actions
                    .get(&path)
                    .is_some_and(|a| a.params.len() == 1)
                {
                    path.clone()
                } else {
                    format!("{path} {param}")
                };
                super::choice_row(
                    ui,
                    &label,
                    &mut field,
                    &[(false, "Slider"), (true, "Field")],
                );
                if field {
                    self.fields.insert(key);
                } else {
                    self.fields.remove(&key);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;

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
        }
    }

    fn named(name: &str, kind: ParamKind) -> Param {
        Param {
            name: name.to_owned(),
            ..param(kind)
        }
    }

    fn harness(actions: Vec<ActionDescriptor>) -> egui_kittest::Harness<'static, State> {
        let mut model = Model::default();
        model.actions.set(actions);
        let mut invoker = Invoker::default();
        invoker.connected = true;
        let state = State {
            view: Actions::new(),
            model,
            invoker,
            time: crate::app::time::TimeRange::default(),
        };
        let mut harness = egui_kittest::Harness::new_ui_state(
            |ui, state: &mut State| {
                let mut context = ViewContext {
                    model: &state.model,
                    time: &mut state.time,
                    invoker: &mut state.invoker,
                    formats: &super::super::NO_FORMATS,
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
            .map(|proto::Request::Invoke(invocation)| (invocation.path, invocation.args))
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
    fn confirmed_actions_are_sent_after_confirming() {
        let reboot = ActionDescriptor {
            confirm: true,
            ..action("reboot", vec![])
        };
        let mut harness = harness(vec![reboot]);
        harness.get_by_label(&format!("{PLAY} Run")).click();
        harness.run_steps(2);
        assert!(sent(&mut harness).is_empty());
        harness.get_by_label("Run").click();
        harness.run_steps(2);
        assert_eq!(sent(&mut harness).len(), 1);
        // the dialog is gone
        assert!(harness.query_by_label("Run").is_none());
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
                        node.label().as_deref() == Some(run.as_str())
                            && node.is_disabled() == disabled
                    })
                    .count()
            };
            (count(true), count(false))
        };
        harness.state_mut().invoker.connected = false;
        harness.run_steps(2);
        assert_eq!(buttons(&harness), (2, 0));
        harness.state_mut().invoker.connected = true;
        harness.run_steps(2);
        assert_eq!(buttons(&harness), (1, 1));
    }

    #[test]
    fn chosen_actions_show_also_when_not_announced() {
        let mut harness = harness(vec![action("a", vec![]), action("b", vec![])]);
        harness.state_mut().view.shown = Some(["b".to_owned(), "gone".to_owned()].into());
        harness.run_steps(2);
        assert!(harness.query_by_label("a").is_none());
        harness.get_by_label("b");
        harness.get_by_label("not announced by the target");
    }
}
