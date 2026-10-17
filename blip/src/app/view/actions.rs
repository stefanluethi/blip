//! A form of the actions the outposts announced, to invoke them.
//!
//! An action without parameters is a button. An action with one parameter is sent as
//! soon as its input changes: a toggle, a choice, a slider while it is dragged (at most
//! every [`DRAG_INTERVAL`](crate::app::model::actions::DRAG_INTERVAL)), a number or text field on Enter. An action with several
//! parameters, or one that asks for confirmation, is sent with its Send button.
//!
//! An action is known by its path, which several outposts may announce. Each action
//! is sent to the outpost chosen for it in the properties, and its readback is the
//! trace chosen there, else the one that outpost announces.

use super::{ViewContext, Viewable, properties_section, short_names};
use crate::app::model::Model;
use crate::app::model::actions::{ActionId, Args, Invoker, Outcome};
use crate::app::model::numbers::{DisplayFormat, Number};
use crate::app::model::traces::Samples;
use egui::RichText;
use egui_phosphor::regular::{CHECK_CIRCLE, CLOCK_COUNTDOWN, PLAY, WARNING_CIRCLE};
use proto::{ActionDescriptor, Data, Param, ParamKind};
use std::collections::{BTreeMap, BTreeSet};

/// Choices of at most this many options are buttons side by side, longer ones a
/// dropdown.
const MAX_BUTTON_OPTIONS: usize = 4;
/// The width of the inputs in points.
const INPUT_WIDTH: f32 = 180.0;
/// The width of the dropdowns in the properties in points.
const PROPERTY_WIDTH: f32 = 140.0;

/// The actions shown, where they are sent, their readbacks and how their inputs look.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct Actions {
    /// The paths of the shown actions, checked in the tree of the left panel.
    #[serde(deserialize_with = "deserialize_shown")]
    pub shown: BTreeSet<String>,
    /// The numeric parameters with bounds entered in a field instead of a slider, by
    /// the path of the action and the parameter name joined with ":".
    pub fields: BTreeSet<String>,
    /// The name of the connection of the outpost each action is sent to, by the path
    /// of the action. An action without one is sent to the first outpost announcing
    /// it, see [`Actions::outpost`].
    pub outposts: BTreeMap<String, String>,
    /// The path of the readback trace of each action by its path, `None` for no
    /// readback. An action without one has the readback its outpost announces, see
    /// [`Actions::readback`].
    pub readbacks: BTreeMap<String, Option<String>>,
}

/// The shown actions as saved. Older versions of blip saved `null` to show all
/// announced actions; such a view shows none until they are checked in the tree.
fn deserialize_shown<'de, D>(deserializer: D) -> Result<BTreeSet<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    Ok(Option::<BTreeSet<String>>::deserialize(deserializer)?.unwrap_or_default())
}

/// A shown action, as the view lists it.
struct Row<'a> {
    path: &'a str,
    /// The outpost the action is sent to, `None` if no outpost announces it.
    outpost: Option<&'a str>,
    /// The action as that outpost announces it, `None` if it does not.
    action: Option<&'a ActionDescriptor>,
}

/// A shown action an outpost announces, as its row shows it.
struct Shown<'a> {
    /// The action, sent to the outpost chosen for it.
    id: ActionId,
    action: &'a ActionDescriptor,
    /// The path of its readback trace.
    readback: Option<&'a str>,
    /// The name it is shown with.
    name: &'a str,
}

/// The name of the connection `source` to show, also when it has none.
fn outpost_name(source: &str) -> &str {
    if source.is_empty() {
        "(unnamed)"
    } else {
        source
    }
}

/// How a change of an input sends its action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Change {
    None,
    /// The input is dragged, send at most every [`DRAG_INTERVAL`](crate::app::model::actions::DRAG_INTERVAL).
    Dragging,
    /// The input was released after dragging, send its value unless sent already.
    Released,
    /// The input got its new value, send it now.
    Done,
}

/// Why `action`, announced as `descriptor` by the outpost it is sent to, cannot be
/// invoked now, if it cannot.
fn disabled_reason(
    invoker: &Invoker,
    action: &ActionId,
    descriptor: &ActionDescriptor,
) -> Option<&'static str> {
    if !invoker.is_connected(action) {
        Some("Not connected to its outpost")
    } else if !descriptor.enabled {
        Some("The outpost cannot carry out the action now")
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

/// The integer in `data`, exactly, or a float rounded.
fn as_integer(data: &Data) -> Option<i128> {
    match *data {
        Data::Uint(value) => Some(value.into()),
        Data::Int(value) => Some(value.into()),
        Data::Float(value) if value.is_finite() => Some(value.round() as i128),
        _ => None,
    }
}

/// An integer within the bounds of a parameter, and within the integers on the wire.
fn clamp_integer(value: i128, min: Option<i64>, max: Option<i64>) -> i128 {
    let min = min.map_or(i64::MIN.into(), i128::from);
    let max = max.map_or(u64::MAX.into(), i128::from);
    // not `clamp`, which panics for bounds the wrong way round
    value.max(min).min(max)
}

/// An integer as data, unsigned unless negative as on the wire.
fn integer(value: i128) -> Data {
    match u64::try_from(value) {
        Ok(value) => Data::Uint(value),
        Err(_) => Data::Int(i64::try_from(value).unwrap_or(i64::MIN)),
    }
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
            Some(integer(clamp_integer(as_integer(data)?, *min, *max)))
        }
        (ParamKind::Float { min, max, .. }, data) => {
            let value = as_f64(data)?;
            let value = value.max(min.unwrap_or(f64::MIN));
            Some(Data::Float(value.min(max.unwrap_or(f64::MAX))))
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
        ParamKind::Int { min, max } => integer(clamp_integer(0, *min, *max)),
        ParamKind::Float { min, .. } => Data::Float(min.unwrap_or(0.0).max(0.0)),
        ParamKind::Enum(options) => options
            .first()
            .map_or(Data::Null, |option| Data::String(option.clone())),
        ParamKind::String => Data::String(String::new()),
    }
}

/// Shows an input for `value` of `param` of the action at `path`. Numbers with both
/// bounds get a slider unless `field`.
fn input_ui(ui: &mut egui::Ui, path: &str, param: &Param, value: &mut Data, field: bool) -> Change {
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
                egui::ComboBox::from_id_salt(ui.id().with(("choice", path, &param.name)))
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
        ParamKind::Int {
            min: Some(low),
            max: Some(high),
        } if !field => {
            let before = as_f64(value).unwrap_or(0.0);
            let mut number = before;
            let bounds = (Some(*low as f64), Some(*high as f64));
            let response = number_ui(ui, &mut number, bounds, Some(1.0), false, &unit_suffix);
            let changed = number != before;
            // the value itself stays exact unless the slider moves
            if changed {
                *value = integer(clamp_integer(
                    number.round() as i128,
                    Some(*low),
                    Some(*high),
                ));
            }
            change_of(&response, changed)
        }
        ParamKind::Int { min, max } => {
            let id = ui.id().with(("integer", path, &param.name));
            integer_field(ui, id, value, (*min, *max), &param.unit)
        }
        ParamKind::Float { min, max, step } => {
            let before = as_f64(value).unwrap_or(0.0);
            let mut number = before;
            let response = number_ui(ui, &mut number, (*min, *max), *step, field, &unit_suffix);
            *value = Data::Float(number);
            change_of(&response, number != before)
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

/// How a slider or number field whose number `changed` changed: dragging while
/// dragged, released when let go without a change, done when typed in, changed with
/// the keys or by the frame that lets go. A click counts as a short drag. The response
/// of a field being typed in reports each key as a change, so whether the number
/// changed is told apart.
fn change_of(response: &egui::Response, changed: bool) -> Change {
    if response.drag_stopped() && !changed {
        Change::Released
    } else if !changed {
        Change::None
    } else if response.dragged() && !response.drag_stopped() {
        Change::Dragging
    } else {
        Change::Done
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
            // a typed number applies on Enter, not with each key
            let mut drag = egui::DragValue::new(number)
                .update_while_editing(false)
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

/// Shows a field to type an integer for `value`, exact also beyond the integers a
/// float holds. The typed number applies on Enter or when leaving the field, clamped
/// to `bounds`; text that is no integer is dropped.
fn integer_field(
    ui: &mut egui::Ui,
    id: egui::Id,
    value: &mut Data,
    (min, max): (Option<i64>, Option<i64>),
    unit: &str,
) -> Change {
    let current = match *value {
        Data::Uint(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        _ => String::new(),
    };
    let mut text: String = ui.data(|data| data.get_temp(id)).unwrap_or(current);
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(id)
            .desired_width(INPUT_WIDTH - 60.0),
    );
    if !unit.is_empty() {
        ui.label(unit);
    }
    if response.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, text));
        return Change::None;
    }
    ui.data_mut(|data| data.remove::<String>(id));
    let entered = response.lost_focus() && !ui.input(|input| input.key_pressed(egui::Key::Escape));
    let Some(number) = text.trim().parse::<i128>().ok().filter(|_| entered) else {
        return Change::None;
    };
    let number = integer(clamp_integer(number, min, max));
    if number == *value {
        return Change::None;
    }
    *value = number;
    Change::Done
}

/// The decimals that show multiples of `step`.
fn decimals(step: f64) -> usize {
    let mut decimals = 0;
    while decimals < 6 && (step * 10f64.powi(decimals as i32)).fract().abs() > 1e-9 {
        decimals += 1;
    }
    decimals
}

/// Shows the outcome of the latest invocation of `action`.
fn outcome_ui(ui: &mut egui::Ui, invoker: &Invoker, action: &ActionId, now: f64) {
    match invoker.outcome(action, now) {
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
    id: ActionId,
    /// The path of its readback trace.
    readback: Option<String>,
    args: Args,
}

impl Actions {
    pub fn new() -> Self {
        Self::default()
    }

    /// The name of the connection of the outpost the action at `path` is sent to: the
    /// one chosen in the properties, also while it does not announce the action, else
    /// the first outpost announcing it, `None` if none does.
    fn outpost<'a>(&'a self, model: &'a Model, path: &str) -> Option<&'a str> {
        match self.outposts.get(path) {
            Some(source) => Some(source),
            None => model
                .actions
                .announcers(path)
                .next()
                .map(|action| action.source.as_str()),
        }
    }

    /// The action at `path` as the outpost it is sent to announces it, if it does.
    fn announced<'a>(&self, model: &'a Model, path: &str) -> Option<&'a ActionDescriptor> {
        let outpost = self.outpost(model, path)?;
        model.actions.announced(outpost, path)
    }

    /// The path of the readback trace of the action at `path`: the one chosen in the
    /// properties, which may be none, else `announced`, the one its outpost announces.
    fn readback<'a>(&'a self, path: &str, announced: Option<&'a str>) -> Option<&'a str> {
        match self.readbacks.get(path) {
            Some(chosen) => chosen.as_deref(),
            None => announced,
        }
    }

    /// The shown actions in the order they were announced first, followed by the
    /// shown actions no outpost announces.
    fn rows<'a>(&'a self, model: &'a Model) -> Vec<Row<'a>> {
        let announced = model
            .actions
            .paths()
            .filter(|path| self.shown.contains(*path));
        let missing = self
            .shown
            .iter()
            .map(String::as_str)
            .filter(|path| model.actions.announcers(path).next().is_none());
        announced
            .chain(missing)
            .map(|path| {
                let outpost = self.outpost(model, path);
                Row {
                    path,
                    outpost,
                    action: outpost.and_then(|outpost| model.actions.announced(outpost, path)),
                }
            })
            .collect()
    }

    fn uses_field(&self, path: &str, param: &str) -> bool {
        self.fields.contains(&format!("{path}:{param}"))
    }

    /// Shows the row of `shown`, with a row per parameter if it has a Send button.
    fn action_ui(
        &self,
        ui: &mut egui::Ui,
        context: &mut ViewContext<'_>,
        shown: &Shown<'_>,
        now: f64,
        confirm: &mut Option<Confirmation>,
    ) {
        let Shown {
            id,
            action,
            readback: readback_path,
            name,
        } = shown;
        let invoker = &mut *context.invoker;
        let disabled = disabled_reason(invoker, id, action);
        let readback =
            readback_path.and_then(|path| Some((path, latest_text(context.model, path)?)));
        let mut hover = format!("{} on {}", action.path, outpost_name(&id.source));
        if !action.description.is_empty() {
            hover = format!("{}\n{hover}", action.description);
        }

        // the inputs, starting from the readback or the defaults
        let readback_samples = context.model.readback_samples(*readback_path);
        let follows_readback = action.params.len() == 1
            && matches!(action.params[0].kind, ParamKind::Bool | ParamKind::Enum(_))
            && invoker.follows_readback(id, readback_samples);
        let readback_value = readback_path.and_then(|path| latest_value(context.model, path));
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
        ui.label(*name).on_hover_text(&hover);
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
                    change = input_ui(ui, &action.path, &action.params[0], &mut values[0], field);
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
        outcome_ui(ui, invoker, id, now);
        ui.end_row();

        if send_button {
            for (param, value) in action.params.iter().zip(&mut values) {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.weak(&param.name);
                });
                ui.add_enabled_ui(disabled.is_none(), |ui| {
                    let field = self.uses_field(&action.path, &param.name);
                    ui.horizontal(|ui| input_ui(ui, &action.path, param, value, field));
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
                id: id.clone(),
                readback: readback_path.map(str::to_owned),
                args,
            });
        } else {
            match change {
                Change::Dragging => invoker.invoke_dragging(id, *readback_path, args, now),
                Change::Released => invoker.finish_dragging(id, *readback_path, args, now),
                _ => invoker.invoke(id, *readback_path, args, now),
            }
        }
    }

    /// Shows the row of a shown action that the outpost it is sent to does not
    /// announce.
    fn missing_ui(ui: &mut egui::Ui, row: &Row<'_>, name: &str) {
        ui.weak(name).on_hover_text(row.path);
        match row.outpost {
            Some(outpost) => ui.weak(format!("not announced by {}", outpost_name(outpost))),
            None => ui.weak("not announced by any outpost"),
        };
        ui.end_row();
    }

    /// Asks to confirm the action held in `confirm`, invoking it if confirmed. Run is
    /// disabled while the action cannot be carried out, e.g. once the connection
    /// ended or the outpost disabled the action while the dialog was open.
    fn confirm_ui(ui: &mut egui::Ui, context: &mut ViewContext<'_>, id: egui::Id, now: f64) {
        let Some(confirmation) = ui.data(|data| data.get_temp::<Confirmation>(id)) else {
            return;
        };
        let invoker = &mut *context.invoker;
        let action = &confirmation.id;
        let disabled = match context
            .model
            .actions
            .announced(&action.source, &action.path)
        {
            Some(descriptor) => disabled_reason(invoker, action, descriptor),
            None => Some("The outpost no longer announces the action"),
        };
        let mut decided = false;
        egui::Modal::new(id.with("modal")).show(ui.ctx(), |ui| {
            ui.set_max_width(320.0);
            ui.label(format!("Carry out {}?", action.path));
            if let Some(reason) = disabled {
                ui.weak(reason);
            }
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if ui
                    .add_enabled(disabled.is_none(), egui::Button::new("Run"))
                    .clicked()
                {
                    let args = confirmation.args.clone();
                    invoker.invoke(action, confirmation.readback.as_deref(), args, now);
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

    /// Shows a dropdown to choose the outpost the action at `path` is sent to among
    /// those announcing it. One chosen before that no longer announces it is listed
    /// greyed out.
    fn outpost_ui(&mut self, ui: &mut egui::Ui, model: &Model, path: &str) {
        let mut sources: Vec<&str> = model
            .actions
            .announcers(path)
            .map(|action| action.source.as_str())
            .collect();
        let current = self.outpost(model, path).map(str::to_owned);
        if let Some(current) = &current
            && !sources.contains(&current.as_str())
        {
            sources.push(current);
        }
        let text = |source: &str| {
            let text = RichText::new(outpost_name(source));
            let announces = model.actions.announced(source, path).is_some();
            if announces { text } else { text.weak() }
        };
        let selected = match &current {
            Some(current) => text(current),
            None => RichText::new("none").weak(),
        };
        let mut chosen = None;
        egui::ComboBox::from_id_salt(("action outpost", path))
            .selected_text(selected)
            .width(PROPERTY_WIDTH)
            .truncate()
            .show_ui(ui, |ui| {
                for &source in &sources {
                    let selected = current.as_deref() == Some(source);
                    if ui.selectable_label(selected, text(source)).clicked() {
                        chosen = Some(source.to_owned());
                    }
                }
            })
            .response
            .on_hover_text("The outpost the action is sent to, by default the first announcing it");
        if let Some(source) = chosen {
            self.outposts.insert(path.to_owned(), source);
        }
    }

    /// Shows a dropdown to choose the readback trace of the action at `path` among the
    /// traces, or none. One chosen before that was not received is listed greyed out.
    /// Choosing the readback the outpost announces follows its announcement again.
    fn readback_ui(&mut self, ui: &mut egui::Ui, model: &Model, path: &str) {
        let announced = self
            .announced(model, path)
            .and_then(|action| action.readback.as_deref());
        let current = self.readback(path, announced);
        let mut options: Vec<Option<&str>> = std::iter::once(None)
            .chain(model.traces.iter().map(|trace| Some(trace.path.as_str())))
            .collect();
        if !options.contains(&current) {
            options.push(current);
        }
        let text = |option: Option<&str>| match option {
            None => RichText::new("None"),
            Some(trace) if model.traces.get(trace).is_some() => RichText::new(trace),
            Some(trace) => RichText::new(trace).weak(),
        };
        let mut chosen = None;
        egui::ComboBox::from_id_salt(("action readback", path))
            .selected_text(text(current))
            .width(PROPERTY_WIDTH)
            .truncate()
            .show_ui(ui, |ui| {
                for &option in &options {
                    if ui.selectable_label(option == current, text(option)).clicked() {
                        chosen = Some(option.map(str::to_owned));
                    }
                }
            })
            .response
            .on_hover_text(
                "The trace showing the state the action sets, by default the one its outpost announces",
            );
        match chosen {
            Some(chosen) if chosen.as_deref() == announced => {
                self.readbacks.remove(path);
            }
            Some(chosen) => {
                self.readbacks.insert(path.to_owned(), chosen);
            }
            None => {}
        }
    }

    /// Shows whether the numbers with a minimum and a maximum of the actions at
    /// `paths` are entered with a slider or a field.
    fn number_inputs_ui(&mut self, ui: &mut egui::Ui, model: &Model, paths: &[String]) {
        let bounded: Vec<(&ActionDescriptor, &str)> = paths
            .iter()
            .filter_map(|path| self.announced(model, path))
            .flat_map(|action| {
                action.params.iter().filter_map(move |param| {
                    let bounded = match param.kind {
                        ParamKind::Int { min, max } => min.is_some() && max.is_some(),
                        ParamKind::Float { min, max, .. } => min.is_some() && max.is_some(),
                        _ => false,
                    };
                    bounded.then_some((action, param.name.as_str()))
                })
            })
            .collect();
        if bounded.is_empty() {
            return;
        }
        let names = short_names(bounded.iter().map(|(action, _)| action.path.as_str()));
        properties_section(ui, "Number inputs", |ui| {
            for ((action, param), name) in bounded.iter().zip(names) {
                let key = format!("{}:{param}", action.path);
                let mut field = self.fields.contains(&key);
                let label = if action.params.len() == 1 {
                    name.to_owned()
                } else {
                    format!("{name} {param}")
                };
                ui.label(label).on_hover_text(&action.path);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut field, false, "Slider");
                    ui.selectable_value(&mut field, true, "Field");
                });
                ui.end_row();
                if field {
                    self.fields.insert(key);
                } else {
                    self.fields.remove(&key);
                }
            }
        });
    }
}

impl Viewable for Actions {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let now = ui.input(|input| input.time);
        let model = context.model;
        let rows = self.rows(model);
        if rows.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.weak(
                    if model.actions.is_empty() && context.invoker.connected.is_empty() {
                        "Connect to an outpost to see its actions"
                    } else if model.actions.is_empty() {
                        "The outposts announced no actions"
                    } else {
                        "Check the actions to show in the tree on the left"
                    },
                );
            });
            return;
        }
        let names = short_names(rows.iter().map(|row| row.path));
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
                            for (row, name) in rows.iter().zip(&names) {
                                let (Some(outpost), Some(action)) = (row.outpost, row.action)
                                else {
                                    Self::missing_ui(ui, row, name);
                                    continue;
                                };
                                let announced = action.readback.as_deref();
                                let shown = Shown {
                                    id: ActionId::new(outpost, row.path),
                                    action,
                                    readback: self.readback(row.path, announced),
                                    name,
                                };
                                self.action_ui(ui, context, &shown, now, &mut confirm);
                            }
                        });
                });
            });
        if let Some(confirmation) = confirm {
            ui.data_mut(|data| data.insert_temp(confirm_id, confirmation));
        }
        Self::confirm_ui(ui, context, confirm_id, now);
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, model: &Model) {
        let paths: Vec<String> = self
            .rows(model)
            .iter()
            .map(|row| row.path.to_owned())
            .collect();
        if paths.is_empty() {
            properties_section(ui, "Actions", |ui| {
                ui.weak("Check the actions to show in the tree on the left");
                ui.end_row();
            });
            return;
        }
        let names = short_names(paths.iter().map(String::as_str));
        properties_section(ui, "Outposts", |ui| {
            for (path, name) in paths.iter().zip(&names) {
                ui.label(*name).on_hover_text(path);
                self.outpost_ui(ui, model, path);
                ui.end_row();
            }
        });
        properties_section(ui, "Readbacks", |ui| {
            for (path, name) in paths.iter().zip(&names) {
                ui.label(*name).on_hover_text(path);
                self.readback_ui(ui, model, path);
                ui.end_row();
            }
        });
        self.number_inputs_ui(ui, model, &paths);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut view = Actions::new();
        view.shown = actions.iter().map(|action| action.path.clone()).collect();
        let mut model = Model::default();
        model.actions.set("", actions);
        let mut invoker = Invoker::default();
        invoker.connected.insert(String::new());
        let state = State {
            view,
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
    fn confirming_is_refused_once_the_action_is_disabled() {
        let reboot = ActionDescriptor {
            confirm: true,
            ..action("reboot", vec![])
        };
        let mut harness = harness(vec![reboot.clone()]);
        harness.get_by_label(&format!("{PLAY} Run")).click();
        harness.run_steps(2);
        // the target disables the action while the dialog is open
        let disabled = ActionDescriptor {
            enabled: false,
            ..reboot
        };
        harness.state_mut().model.actions.set("", vec![disabled]);
        harness.run_steps(2);
        harness.get_by_label("Run").click();
        harness.run_steps(2);
        assert!(sent(&mut harness).is_empty());
        harness.get_by_label("The outpost cannot carry out the action now");
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
        // an action whose chosen outpost does not announce it
        let view = &mut harness.state_mut().view;
        view.outposts.insert("b".to_owned(), "robby".to_owned());
        harness.run_steps(2);
        harness.get_by_label("not announced by robby");
    }

    #[test]
    fn only_the_actions_checked_in_the_tree_show() {
        let mut harness = harness(vec![action("a", vec![])]);
        harness.state_mut().view.shown.clear();
        harness.run_steps(2);
        harness.get_by_label("Check the actions to show in the tree on the left");
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

        harness.state_mut().view.outposts = [("reset".to_owned(), "b".to_owned())].into();
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
    fn the_chosen_readback_shows_and_starts_the_inputs() {
        let readback = |path: &str| ActionDescriptor {
            readback: Some(path.to_owned()),
            ..action("enable", vec![named("on", ParamKind::Bool)])
        };
        let mut harness = harness(vec![readback("announced")]);
        let model = &mut harness.state_mut().model;
        for (path, value) in [("announced", false), ("chosen", true)] {
            model.traces.append(&proto::Value {
                timestamp: 0,
                path: path.to_owned(),
                value: Data::Bool(value),
            });
        }
        let checked = |harness: &egui_kittest::Harness<'_, State>| {
            let checkbox = harness.get_by_role(egui::accesskit::Role::CheckBox);
            checkbox.accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
        };
        harness.run_steps(2);
        harness.get_by_label("false");
        assert!(!checked(&harness));

        let view = &mut harness.state_mut().view;
        view.readbacks
            .insert("enable".to_owned(), Some("chosen".to_owned()));
        harness.run_steps(2);
        harness.get_by_label("true");
        assert!(checked(&harness));

        // no readback
        let view = &mut harness.state_mut().view;
        view.readbacks.insert("enable".to_owned(), None);
        harness.run_steps(2);
        assert!(harness.query_by_label("true").is_none());
        assert!(harness.query_by_label("false").is_none());
    }

    /// A harness showing the properties of an actions view showing "speed", which the
    /// connections "a" and "b" announce, "a" with the readback "rpm", and the traces
    /// "rpm" and "temp".
    fn properties_harness() -> egui_kittest::Harness<'static, (Actions, Model)> {
        let mut model = Model::default();
        let speed = |readback: Option<&str>| ActionDescriptor {
            readback: readback.map(str::to_owned),
            ..action("speed", vec![])
        };
        model.actions.set("a", vec![speed(Some("rpm"))]);
        model.actions.set("b", vec![speed(None)]);
        for path in ["rpm", "temp"] {
            model.traces.append(&proto::Value {
                timestamp: 0,
                path: path.to_owned(),
                value: Data::Uint(1),
            });
        }
        let mut view = Actions::new();
        view.shown.insert("speed".to_owned());
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(300.0, 600.0))
            .build_ui_state(
                |ui, (view, model): &mut (Actions, Model)| view.properties_ui(ui, model),
                (view, model),
            );
        harness.run();
        harness
    }

    /// Opens the dropdown showing `selected` and clicks its option `option`.
    fn choose(
        harness: &mut egui_kittest::Harness<'_, (Actions, Model)>,
        selected: &str,
        option: &str,
    ) {
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

    #[test]
    fn the_outpost_of_an_action_is_chosen_among_those_announcing_it() {
        let mut harness = properties_harness();
        choose(&mut harness, "a", "b");
        let view = &harness.state().0;
        assert_eq!(view.outposts.get("speed").map(String::as_str), Some("b"));
        // the readback follows the outpost, which announces none
        harness.get_by(|node| {
            node.role() == egui::accesskit::Role::ComboBox
                && node.value().as_deref() == Some("None")
        });

        // a chosen outpost that is gone is still listed
        harness.state_mut().0.outposts = [("speed".to_owned(), "gone".to_owned())].into();
        harness.run();
        choose(&mut harness, "gone", "a");
        let view = &harness.state().0;
        assert_eq!(view.outposts.get("speed").map(String::as_str), Some("a"));
    }

    #[test]
    fn the_readback_of_an_action_is_chosen_among_the_traces() {
        let mut harness = properties_harness();
        choose(&mut harness, "rpm", "temp");
        let readbacks = |harness: &egui_kittest::Harness<'_, (Actions, Model)>| {
            harness.state().0.readbacks.clone()
        };
        assert_eq!(
            readbacks(&harness),
            [("speed".to_owned(), Some("temp".to_owned()))].into()
        );
        choose(&mut harness, "temp", "None");
        assert_eq!(readbacks(&harness), [("speed".to_owned(), None)].into());
        // choosing the announced readback follows the announcement again
        choose(&mut harness, "None", "rpm");
        assert!(readbacks(&harness).is_empty());
    }

    #[test]
    fn layouts_of_older_versions_load() {
        let all: Actions = serde_json::from_str(r#"{"shown": null, "fields": []}"#).unwrap();
        assert!(all.shown.is_empty() && all.outposts.is_empty());
        let chosen: Actions = serde_json::from_str(r#"{"shown": ["a"]}"#).unwrap();
        assert_eq!(chosen.shown, ["a".to_owned()].into());

        let mut view = Actions::new();
        view.outposts.insert("a".to_owned(), "robby".to_owned());
        view.readbacks.insert("a".to_owned(), None);
        view.readbacks
            .insert("b".to_owned(), Some("rpm".to_owned()));
        let json = serde_json::to_string(&view).unwrap();
        let loaded: Actions = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.outposts, view.outposts);
        assert_eq!(loaded.readbacks, view.readbacks);
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
}
