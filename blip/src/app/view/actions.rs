//! A form of the actions the outposts announced, to invoke them.
//!
//! An action without parameters is a button. An action with one parameter is sent as
//! soon as its input changes: a toggle, a choice, a slider while it is dragged (at most
//! every [`DRAG_INTERVAL`](crate::app::model::actions::DRAG_INTERVAL)), a number or text field on Enter. An action with several
//! parameters, or one that asks for confirmation, is sent with its Send button, or one
//! with several parameters, if chosen in the properties, when one of its inputs changes,
//! at most every [`DRAG_INTERVAL`](crate::app::model::actions::DRAG_INTERVAL). An action that
//! asks for confirmation is selected with its button and carried out with a second
//! one in its row, within [`ARM_SECONDS`].
//!
//! The readback of a slider is a needle on it, and the readback of a toggle or choice
//! shows in the input itself, its text only while it differs.
//!
//! An action is known by its path, which several outposts may announce. The actions
//! are sent to the outpost chosen in the properties, by default each to the first
//! outpost announcing it, and their readback is the trace that outpost announces
//! unless it is turned off there.

use super::{PropertiesContext, ViewContext, Viewable, properties_section, short_names};
use crate::app::model::Model;
use crate::app::model::actions::{ActionId, Args, Invoker, Outcome};
use crate::app::model::numbers::{DisplayFormat, Number, SHOWN_DIGITS};
use crate::app::model::traces::Samples;
use crate::app::theme::{self, TokensExt};
use egui::RichText;
use egui_phosphor::regular::{CHECK_CIRCLE, CLOCK_COUNTDOWN, PLAY, WARNING_CIRCLE};
use proto::{ActionDescriptor, Data, Param, ParamKind};
use std::collections::{BTreeSet, HashSet};

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
    /// The paths of the shown actions, checked in the tree of the left panel. Layouts
    /// of older versions that showed all actions show none.
    #[serde(deserialize_with = "null_as_empty")]
    pub shown: BTreeSet<String>,
    /// The numeric parameters with bounds entered in a field instead of a slider, by
    /// the path of the action and the parameter name joined with ":".
    pub fields: BTreeSet<String>,
    /// The name of the connection of the outpost the actions are sent to, `None` to
    /// send each to the first outpost announcing it, see [`Actions::outpost`].
    pub outpost: Option<String>,
    /// The paths of the actions whose readback, the trace their outpost announces, is
    /// turned off.
    pub readbacks_off: BTreeSet<String>,
    /// The paths of the actions with several parameters sent when one of their inputs
    /// changes instead of with their Send button, see [`can_send_on_change`].
    pub on_change: BTreeSet<String>,
}

/// A set of paths, empty for `null`.
fn null_as_empty<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeSet<String>, D::Error> {
    let paths: Option<BTreeSet<String>> = serde::Deserialize::deserialize(deserializer)?;
    Ok(paths.unwrap_or_default())
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
    /// The input is held without changing it while dragged, send nothing.
    Held,
    /// A number is typed into the field but not entered yet, send nothing.
    Typing,
    /// The input is dragged, send at most every [`DRAG_INTERVAL`](crate::app::model::actions::DRAG_INTERVAL).
    Dragging,
    /// The input was released after dragging, send its value unless sent already.
    Released,
    /// The input got its new value, send it now.
    Done,
}

/// Why actions cannot be carried out while not recording.
const START_RECORDING: &str = "Start recording to carry out actions";

/// Why `action`, announced as `descriptor` by the outpost it is sent to, cannot be
/// invoked now, if it cannot: not while its outpost is not connected, nor while not
/// recording, so that the recording shows what it did.
fn disabled_reason(
    invoker: &Invoker,
    action: &ActionId,
    descriptor: &ActionDescriptor,
) -> Option<String> {
    if !invoker.is_connected(action) {
        Some("Not connected to its outpost".to_owned())
    } else if !invoker.recording {
        Some(START_RECORDING.to_owned())
    } else if !descriptor.enabled {
        Some(match target_reason(descriptor) {
            Some(reason) => format!("The outpost cannot carry out the action now: {reason}"),
            None => "The outpost cannot carry out the action now".to_owned(),
        })
    } else {
        None
    }
}

/// Why the target cannot carry out the action of `descriptor` now, if it is disabled
/// and the target says.
fn target_reason(descriptor: &ActionDescriptor) -> Option<&str> {
    descriptor
        .disabled_reason
        .as_deref()
        .filter(|reason| !descriptor.enabled && !reason.is_empty())
}

/// Whether the action lists its parameters below its row, to be sent together.
fn lists_params(action: &ActionDescriptor) -> bool {
    action.params.len() > 1 || (action.confirm && !action.params.is_empty())
}

/// Whether the action can be chosen to be sent when one of its inputs changes instead
/// of with its Send button: one with several parameters that asks for no confirmation.
fn can_send_on_change(action: &ActionDescriptor) -> bool {
    action.params.len() > 1 && !action.confirm
}

/// The latest value of the readback trace at `path`, if it is a number, bool or string.
fn latest_value(model: &Model, path: &str) -> Option<Data> {
    let trace = model.readback(path)?;
    let last = trace.timestamps.len().checked_sub(1)?;
    match &trace.samples {
        Samples::Numbers(numbers) => match numbers.get(last) {
            Number::Null => None,
            Number::Int(value) => Some(Data::Int(value)),
            Number::Uint(value) => Some(Data::Uint(value)),
            Number::Float(value) => Some(Data::Float(value)),
            Number::Bool(value) => Some(Data::Bool(value)),
        },
        Samples::Strings(texts) => texts.get(last).map(Data::from),
        _ => None,
    }
}

/// The latest value of the readback trace at `path` as text with its unit.
fn latest_text(model: &Model, path: &str) -> Option<String> {
    let trace = model.readback(path)?;
    let last = trace.timestamps.len().checked_sub(1)?;
    let value = trace
        .samples
        .format(last, 4, Some(SHOWN_DIGITS), DisplayFormat::Default, None);
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

/// The readback of a number drawn onto its slider.
#[derive(Clone, Copy)]
struct Needle {
    value: f64,
    /// Whether the outpost is connected, so the value is current.
    live: bool,
}

/// Shows an input for `value` of `param` of the action at `path`. Numbers with both
/// bounds get a slider unless `field`, with the readback as a needle on it if any.
fn input_ui(
    ui: &mut egui::Ui,
    path: &str,
    param: &Param,
    value: &mut Data,
    field: bool,
    needle: Option<Needle>,
) -> Change {
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
                        let chosen = selected == *option;
                        if ui.add(theme::selectable(chosen, option)).clicked() && !chosen {
                            selected.clone_from(option);
                            changed = true;
                        }
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
            let response = number_ui(
                ui,
                &mut number,
                bounds,
                Some(1.0),
                false,
                &unit_suffix,
                needle,
            );
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
            let response = number_ui(
                ui,
                &mut number,
                (*min, *max),
                *step,
                field,
                &unit_suffix,
                needle,
            );
            *value = Data::Float(number);
            change_of(&response, number != before)
        }
        ParamKind::String => {
            let mut text = match value {
                Data::String(text) => text.clone(),
                _ => String::new(),
            };
            let field = theme::field(&mut text).desired_width(INPUT_WIDTH);
            let response = ui.add(field);
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
/// dragged, held while pressed or dragged without a change, released when let go
/// without a change, done when typed in, changed with
/// the keys or by the frame that lets go. A click counts as a short drag. The response
/// of a field being typed in reports each key as a change, so whether the number
/// changed is told apart.
fn change_of(response: &egui::Response, changed: bool) -> Change {
    if response.drag_stopped() && !changed {
        Change::Released
    } else if !changed && response.dragged() {
        Change::Held
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
    needle: Option<Needle>,
) -> egui::Response {
    match (min, max) {
        (Some(min), Some(max)) if !field => {
            ui.spacing_mut().slider_width = INPUT_WIDTH - 60.0;
            let mut slider = egui::Slider::new(number, min..=max).suffix(suffix);
            if let Some(step) = step {
                slider = slider.step_by(step);
            }
            let response = ui.add(slider);
            if let Some(needle) = needle {
                needle_ui(ui, &response, (min, max), *number, needle);
            }
            response
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

/// Draws the readback `needle` onto the slider of `response`, whose handle is at
/// `number` within `min` and `max`: a line where the readback is and the gap to the
/// handle shaded, as the setpoint and process value of a controller. Grey while the
/// outpost is not connected.
fn needle_ui(
    ui: &egui::Ui,
    response: &egui::Response,
    (min, max): (f64, f64),
    number: f64,
    needle: Needle,
) {
    let tokens = ui.tokens();
    let color = if needle.live {
        tokens.accent_text
    } else {
        tokens.text_faint
    };
    // the geometry of egui's slider: the rail is as wide as the slider, the centre of
    // the handle moves within it inset by half the handle's width
    let height = response.rect.height();
    let slider = egui::Rect::from_min_size(
        response.rect.min,
        egui::vec2(ui.spacing().slider_width, height),
    );
    let half_handle = match ui.visuals().handle_shape {
        egui::style::HandleShape::Circle => height / 2.5,
        egui::style::HandleShape::Rect { aspect_ratio } => height / 2.5 * aspect_ratio,
    };
    let positions = slider.x_range().shrink(half_handle);
    let x = |value: f64| {
        let fraction = if max > min {
            ((value - min) / (max - min)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        egui::lerp(positions, fraction as f32)
    };
    let (handle, readback) = (x(number), x(needle.value));
    let rail = ui.spacing().slider_rail_height / 2.0;
    let y = slider.center().y;
    // on the rail, which egui paints over anything below it, up to the handle's edge
    let gap = if readback > handle {
        handle + half_handle..=readback
    } else {
        readback..=handle - half_handle
    };
    if gap.start() < gap.end() {
        let shaded = egui::Rect::from_x_y_ranges(gap, y - rail..=y + rail);
        ui.painter()
            .rect_filled(shaded, rail, color.gamma_multiply(0.35));
    }
    let line = egui::Rect::from_center_size(egui::pos2(readback, y), egui::vec2(2.0, height * 0.8));
    ui.painter().rect_filled(line, 1.0, color);
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
    let mut text: String = ui
        .data(|data| data.get_temp(id))
        .unwrap_or_else(|| current.clone());
    let response = ui.add(
        theme::field(&mut text)
            .id(id)
            .desired_width(INPUT_WIDTH - 60.0),
    );
    if !unit.is_empty() {
        ui.label(unit);
    }
    if response.has_focus() {
        let typed = text.trim() != current;
        ui.data_mut(|data| data.insert_temp(id, text));
        return if typed { Change::Typing } else { Change::None };
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

/// The whole seconds since the latest invocation of `action` was sent.
fn seconds_waiting(invoker: &Invoker, action: &ActionId, now: f64) -> u64 {
    invoker
        .latest(action)
        .map_or(0, |latest| (now - latest.sent_at).max(0.0) as u64)
}

/// Shows the outcome of the latest invocation of `action`, or a steady dot while its
/// input is `held` and dragged, instead of the outcome of each value sent meanwhile.
fn outcome_ui(ui: &mut egui::Ui, invoker: &Invoker, action: &ActionId, held: bool, now: f64) {
    if held {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        let color = ui.visuals().weak_text_color();
        ui.painter().circle_filled(rect.center(), 3.0, color);
        response.on_hover_text("Sending while you drag");
        return;
    }
    match invoker.outcome(action, now) {
        None => {
            ui.label("");
        }
        Some(Outcome::Pending) => {
            // the seconds after the first, so a slow action reads as busy, not stuck
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(12.0));
                let seconds = seconds_waiting(invoker, action, now);
                if seconds >= 1 {
                    ui.weak(format!("{seconds} s"));
                }
            })
            .response
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
                .on_hover_text("No answer yet, a late answer still counts");
        }
    }
}

/// What to say below the rows of `action` about its latest invocation, with its
/// colour: why it was rejected, or how long the answer is overdue.
fn note(
    ui: &egui::Ui,
    invoker: &Invoker,
    action: &ActionId,
    descriptor: &ActionDescriptor,
    now: f64,
) -> Option<(String, egui::Color32)> {
    let latest = invoker.latest(action)?;
    match &latest.outcome {
        Outcome::Rejected(reason) => {
            let value = match (descriptor.params.as_slice(), latest.args.as_slice()) {
                ([param], [(_, value)]) if param.kind != ParamKind::Bool => {
                    Some(value_text(value, &param.unit))
                }
                _ => None,
            };
            let text = match value {
                Some(value) => format!("{value} rejected: {reason}"),
                None => format!("Rejected: {reason}"),
            };
            Some((text, ui.visuals().error_fg_color))
        }
        Outcome::Unanswered => {
            let seconds = seconds_waiting(invoker, action, now);
            let text = format!("No answer for {seconds} s, a late answer still counts");
            Some((text, ui.visuals().warn_fg_color))
        }
        _ => None,
    }
}

/// An argument as text with the unit of its parameter.
fn value_text(value: &Data, unit: &str) -> String {
    let text = match value {
        Data::Uint(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        Data::Float(value) => value.to_string(),
        Data::String(text) => text.clone(),
        other => format!("{other:?}"),
    };
    if unit.is_empty() {
        text
    } else {
        format!("{text} {unit}")
    }
}

/// Whether each input of `action`, with `values`, differs from what was sent last, or
/// from the value it started from if the action was not sent yet.
fn unsent(
    invoker: &Invoker,
    action: &ActionId,
    descriptor: &ActionDescriptor,
    values: &[Data],
    readback: Option<&Data>,
) -> Vec<bool> {
    let sent = invoker.latest(action).map(|latest| &latest.args);
    let single = descriptor.params.len() == 1;
    descriptor
        .params
        .iter()
        .zip(values)
        .map(|(param, value)| {
            let before = sent
                .and_then(|args| args.iter().find(|(name, _)| *name == param.name))
                .and_then(|(_, data)| fit(param, data))
                .unwrap_or_else(|| initial(param, readback.filter(|_| single).cloned()));
            *value != before
        })
        .collect()
}

/// Shows a dot by an input changed since it was sent.
fn unsent_dot(ui: &mut egui::Ui) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(6.0, 6.0), egui::Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 3.0, ui.visuals().text_color());
    response
        .on_hover_text("Changed since it was sent")
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Changed"));
}

/// Shows the readback text in the colour of live data after a bar like the needle on
/// sliders, greyed out as the last value received while the outpost of the action is
/// not `connected`.
fn readback_ui(ui: &mut egui::Ui, readback: Option<(&str, String)>, connected: bool) {
    let Some((path, text)) = readback else {
        ui.label("");
        return;
    };
    let tokens = ui.tokens();
    let (color, hover) = if connected {
        (tokens.accent_text, format!("Readback of {path}"))
    } else {
        let hover =
            format!("Readback of {path}, the last value received before the connection closed");
        (tokens.text_faint, hover)
    };
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(2.0, 12.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 1.0, color);
        ui.colored_label(color, text)
    })
    .response
    .on_hover_text(hover);
}

/// The width in points up to which a note wraps.
const NOTE_WIDTH: f32 = 420.0;
/// The width in points of the rule along the parameters of an action listing them.
const RULE_WIDTH: f32 = 2.0;
/// The points from the rule's left edge to the names of the parameters beside it.
const RULE_INDENT: f32 = 12.0;
/// The points between the rows of one action.
const ROW_GAP: f32 = 4.0;
/// The points added between actions to the gap between rows, so that the rows of an
/// action read as one.
const ACTION_GAP: f32 = 16.0 - 2.0 * ROW_GAP;

/// Shows `text` in the input column of a row of its own, overflowing into the columns
/// to its right without widening the input column.
fn note_ui(ui: &mut egui::Ui, (text, color): (String, egui::Color32)) {
    ui.label("");
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui.painter().layout(text.clone(), font, color, NOTE_WIDTH);
    let size = egui::vec2(0.0, galley.size().y);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().galley(rect.min, galley, color);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
    ui.end_row();
}

/// Seconds an action that asks for confirmation stays selected to be carried out.
const ARM_SECONDS: f64 = 6.0;

/// What became of an action selected to be carried out, in a frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Armed {
    Waiting,
    Cancelled,
    Confirmed,
}

/// Shows the buttons of an action selected to be carried out: Cancel in the place of
/// the button that selected it, so that a double click cancels, then the button named
/// `label` that carries it out, and below them a line that shortens to nothing as the
/// selection lapses, the fraction `left` of it still to go.
fn armed_ui(ui: &mut egui::Ui, label: &str, left: f32) -> egui::InnerResponse<Armed> {
    ui.vertical(|ui| {
        let mut armed = Armed::Waiting;
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                armed = Armed::Cancelled;
            }
            if ui.button(label).clicked() {
                armed = Armed::Confirmed;
            }
        });
        let width = ui.min_rect().width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 2.0), egui::Sense::hover());
        let to_go = egui::Rect::from_min_size(rect.min, egui::vec2(width * left, 2.0));
        let painter = ui.painter();
        painter.rect_filled(rect, 1.0, ui.tokens().border);
        painter.rect_filled(to_go, 1.0, ui.visuals().weak_text_color());
        armed
    })
}

impl Actions {
    /// The name of the connection of the outpost the action at `path` is sent to: the
    /// one chosen in the properties, also while it does not announce the action, else
    /// the first outpost announcing it whose connection is open, else the first
    /// announcing it, `None` if none does. So the actions keep working when the
    /// connection changes to an outpost of another name, while the actions of the one
    /// before are still known.
    fn outpost<'a>(
        &'a self,
        model: &'a Model,
        connected: &HashSet<String>,
        path: &str,
    ) -> Option<&'a str> {
        if let Some(source) = &self.outpost {
            return Some(source);
        }
        let announcers = || {
            model
                .actions
                .announcers(path)
                .map(|action| action.source.as_str())
        };
        announcers()
            .find(|source| connected.contains(*source))
            .or_else(|| announcers().next())
    }

    /// The action at `path` as the outpost it is sent to announces it, if it does.
    fn announced<'a>(
        &self,
        model: &'a Model,
        connected: &HashSet<String>,
        path: &str,
    ) -> Option<&'a ActionDescriptor> {
        let outpost = self.outpost(model, connected, path)?;
        model.actions.announced(outpost, path)
    }

    /// The path of the readback trace of `action`: the one its outpost announces,
    /// unless turned off in the properties.
    fn readback<'a>(&self, action: &'a ActionDescriptor) -> Option<&'a str> {
        if self.readbacks_off.contains(&action.path) {
            None
        } else {
            action.readback.as_deref()
        }
    }

    /// The shown actions in the order they were announced first, followed by the
    /// shown actions no outpost announces.
    fn rows<'a>(&'a self, model: &'a Model, connected: &HashSet<String>) -> Vec<Row<'a>> {
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
                let outpost = self.outpost(model, connected, path);
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

    /// Whether `action`, which lists its parameters, is sent when one of its inputs
    /// changes rather than with its Send button.
    fn sends_on_change(&self, action: &ActionDescriptor) -> bool {
        can_send_on_change(action) && self.on_change.contains(&action.path)
    }

    /// Shows the row of `shown`, with a row per parameter if it has several or asks
    /// for confirmation, followed by its Send button unless sent on change.
    fn action_ui(
        &self,
        ui: &mut egui::Ui,
        context: &mut ViewContext<'_>,
        shown: &Shown<'_>,
        now: f64,
    ) {
        let Shown {
            id,
            action,
            readback: readback_path,
            name,
        } = shown;
        let invoker = &mut *context.invoker;
        let disabled = disabled_reason(invoker, id, action);
        let mut readback =
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
                let key = (id.clone(), param.name.clone());
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
        // a checkbox or choice shows the readback itself: the text only says when the
        // target reports another value, e.g. right after a change
        if let ([param], Some(value)) = (action.params.as_slice(), &readback_value)
            && matches!(param.kind, ParamKind::Bool | ParamKind::Enum(_))
            && let Some(value) = fit(param, value)
        {
            readback = readback.filter(|_| value != values[0]).map(|(path, _)| {
                let text = match value {
                    Data::Bool(true) => "reads on".to_owned(),
                    Data::Bool(false) => "reads off".to_owned(),
                    Data::String(text) => format!("reads {text}"),
                    other => format!("reads {other:?}"),
                };
                (path, text)
            });
        }

        // what the inputs show before they change this frame
        let values_before = values.clone();

        let listed = lists_params(action);
        let on_change = self.sends_on_change(action);
        let unsent = unsent(invoker, id, action, &values, readback_value.as_ref());
        // inputs that send only when told show what was not sent yet: those of a form
        // with a Send button, and a text field until Enter
        // a number typed into a field and not entered yet, as of the frame before, as
        // the names show before the inputs
        let typing_ids: Vec<egui::Id> = action
            .params
            .iter()
            .map(|param| ui.id().with(("typing", &id.source, &id.path, &param.name)))
            .collect();
        let typing_before: Vec<bool> = typing_ids
            .iter()
            .map(|&typing| ui.data(|data| data.get_temp::<bool>(typing)) == Some(true))
            .collect();
        let mut typing_now = vec![false; action.params.len()];
        let single_text =
            !listed && action.params.first().map(|param| &param.kind) == Some(&ParamKind::String);
        let entering =
            !listed && ((single_text && unsent[0]) || typing_before.first() == Some(&true));
        let marks_unsent = listed && !on_change;
        // an action that asks for confirmation is selected with its button, then carried
        // out with another within ARM_SECONDS; the selection lapses with Escape, once
        // the action cannot be carried out or when an input of it changes
        let armed_id = ui.id().with(("armed", &id.source, &id.path));
        let mut armed_at = ui
            .data(|data| data.get_temp::<f64>(armed_id))
            .filter(|at| now - at < ARM_SECONDS && disabled.is_none())
            .filter(|_| !ui.input(|input| input.key_pressed(egui::Key::Escape)));
        let confirm = |ui: &mut egui::Ui, armed_at: &mut Option<f64>, verb: &str| {
            let at = (*armed_at)?;
            let left = (1.0 - (now - at) / ARM_SECONDS) as f32;
            let armed = armed_ui(ui, &format!("{verb} {name}"), left);
            if armed.inner != Armed::Waiting {
                *armed_at = None;
            }
            Some((armed.inner == Armed::Confirmed, armed.response))
        };
        let mut change = Change::None;
        // at the top of a row taller than a control, e.g. while selected to confirm
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(theme::CONTROL_HEIGHT);
                ui.label(*name).on_hover_text(&hover);
                if entering {
                    unsent_dot(ui);
                }
            });
        });
        ui.add_enabled_ui(disabled.is_none(), |ui| {
            let input = if action.params.is_empty() {
                if let Some((confirmed, response)) = confirm(ui, &mut armed_at, "Run") {
                    if confirmed {
                        change = Change::Done;
                    }
                    response
                } else {
                    let ellipsis = if action.confirm { "…" } else { "" };
                    let button = ui.button(format!("{PLAY} Run{ellipsis}"));
                    if button.clicked() {
                        if action.confirm {
                            armed_at = Some(now);
                        } else {
                            change = Change::Done;
                        }
                    }
                    button
                }
            } else if !listed {
                let field = self.uses_field(&action.path, &action.params[0].name);
                ui.horizontal(|ui| {
                    let needle = readback_value
                        .as_ref()
                        .and_then(as_f64)
                        .map(|value| Needle {
                            value,
                            live: invoker.is_connected(id),
                        });
                    let param = &action.params[0];
                    change = input_ui(ui, &action.path, param, &mut values[0], field, needle);
                    typing_now[0] = change == Change::Typing;
                    if entering {
                        ui.weak("Enter to send");
                    }
                })
                .response
            } else {
                ui.label("")
            };
            if let Some(reason) = &disabled {
                input.on_disabled_hover_text(reason);
            }
        });
        readback_ui(ui, readback, invoker.is_connected(id));
        // whether an input sending while dragged is held, as of the frame before for
        // inputs listed below this row
        let sends_dragged = !listed || on_change;
        let held_id = ui.id().with(("held", &id.source, &id.path));
        let held_before = ui.data(|data| data.get_temp::<bool>(held_id)) == Some(true);
        let held_now = sends_dragged && matches!(change, Change::Held | Change::Dragging);
        outcome_ui(ui, invoker, id, held_before || held_now, now);
        ui.end_row();

        if listed {
            // a rule in the column of the names, from the first parameter to Send, holds
            // the rows of the action together; its inputs stay in the column of inputs
            let mut rule: Option<(f32, egui::Rangef)> = None;
            let extend = |rule: &mut Option<(f32, egui::Rangef)>, x: f32, rows: egui::Rangef| {
                *rule = Some(match *rule {
                    Some((x, span)) => (
                        x,
                        egui::Rangef::new(span.min.min(rows.min), span.max.max(rows.max)),
                    ),
                    None => (x, rows),
                });
            };
            let inputs = action.params.iter().zip(&mut values).zip(&mut typing_now);
            for (index, ((param, value), typing)) in inputs.enumerate() {
                let label = ui.horizontal(|ui| {
                    ui.add_space(RULE_INDENT);
                    ui.weak(&param.name);
                    if marks_unsent && (unsent[index] || typing_before[index]) {
                        unsent_dot(ui);
                    }
                });
                let input = ui.add_enabled_ui(disabled.is_none(), |ui| {
                    let field = self.uses_field(&action.path, &param.name);
                    let changed = ui
                        .horizontal(|ui| input_ui(ui, &action.path, param, value, field, None))
                        .inner;
                    *typing = changed == Change::Typing;
                    if on_change && changed != Change::None {
                        change = changed;
                    }
                    if !matches!(changed, Change::None | Change::Held | Change::Typing) {
                        armed_at = None;
                    }
                });
                let rows = input.response.rect.y_range();
                extend(&mut rule, label.response.rect.left(), rows);
                ui.label("");
                ui.label("");
                ui.end_row();
            }
            if !on_change {
                ui.label("");
                let send = ui.add_enabled_ui(disabled.is_none(), |ui| {
                    if let Some((confirmed, _)) = confirm(ui, &mut armed_at, "Send") {
                        if confirmed {
                            change = Change::Done;
                        }
                    } else if action.confirm {
                        if ui.button("Send…").clicked() {
                            armed_at = Some(now);
                        }
                    } else if ui.button("Send").clicked() {
                        change = Change::Done;
                    }
                });
                if let Some((x, _)) = rule {
                    extend(&mut rule, x, send.response.rect.y_range());
                }
                ui.label("");
                ui.label("");
                ui.end_row();
            }
            if let Some((x, span)) = rule {
                let stroke = egui::Stroke::new(RULE_WIDTH, ui.tokens().border_strong);
                ui.painter().vline(x + RULE_WIDTH / 2.0, span, stroke);
            }
        }

        let held = sends_dragged && matches!(change, Change::Held | Change::Dragging);
        ui.data_mut(|data| {
            data.insert_temp(held_id, held);
            for (&typing_id, &typing) in typing_ids.iter().zip(&typing_now) {
                data.insert_temp(typing_id, typing);
            }
        });
        // why the target cannot carry it out now, if it says, while connected
        if invoker.is_connected(id)
            && let Some(reason) = target_reason(action)
        {
            note_ui(ui, (reason.to_owned(), ui.visuals().weak_text_color()));
        }
        match armed_at {
            Some(at) => {
                ui.data_mut(|data| data.insert_temp(armed_id, at));
                ui.ctx().request_repaint_after_secs(0.05);
                if !action.description.is_empty() {
                    note_ui(
                        ui,
                        (action.description.clone(), ui.visuals().weak_text_color()),
                    );
                }
            }
            None => ui.data_mut(|data| data.remove::<f64>(armed_id)),
        }
        if !held
            && !held_before
            && let Some(note) = note(ui, invoker, id, action, now)
        {
            note_ui(ui, note);
        }

        // when the target rejects a value, an input that sends at once goes back to
        // the value it accepted last, as it shows what the target holds, once let go
        // if dragged; a text field keeps the text to correct it, and inputs listed
        // below the row keep theirs as a draft to send again
        let let_go = !held && !invoker.is_dragging(id);
        if let_go && !listed && change == Change::None && invoker.take_rejected(id) {
            let accepted = invoker.accepted(id);
            for (param, value) in action.params.iter().zip(&mut values) {
                if param.kind == ParamKind::String {
                    continue;
                }
                *value = accepted
                    .and_then(|args| args.iter().find(|(name, _)| *name == param.name))
                    .and_then(|(_, data)| fit(param, data))
                    .unwrap_or_else(|| initial(param, readback_value.clone()));
            }
        }

        for (param, value) in action.params.iter().zip(&values) {
            invoker
                .inputs
                .insert((id.clone(), param.name.clone()), value.clone());
        }
        if disabled.is_some() || matches!(change, Change::None | Change::Held | Change::Typing) {
            return;
        }
        let names = || action.params.iter().map(|param| param.name.clone());
        invoker.assume_accepted(id, names().zip(values_before).collect());
        let args: Args = names().zip(values).collect();
        if on_change {
            // each change at most every DRAG_INTERVAL, the latest held back until due
            match change {
                Change::Released => invoker.finish_dragging(id, *readback_path, args, now),
                _ => invoker.invoke_dragging(id, *readback_path, args, now),
            }
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
        ui.horizontal(|ui| {
            ui.set_min_height(theme::CONTROL_HEIGHT);
            ui.weak(name).on_hover_text(row.path);
        });
        match row.outpost {
            Some(outpost) => ui.weak(format!("not announced by {}", outpost_name(outpost))),
            None => ui.weak("not announced by any outpost"),
        };
        ui.end_row();
    }

    /// Shows a dropdown to choose the outpost the actions are sent to among those that
    /// announced actions, or for each the first announcing it. One chosen before that
    /// announces none is listed greyed out.
    fn outpost_ui(&mut self, ui: &mut egui::Ui, model: &Model) {
        let current = self.outpost.as_deref();
        let mut options: Vec<Option<&str>> = std::iter::once(None)
            .chain(model.actions.sources().map(Some))
            .collect();
        if !options.contains(&current) {
            options.push(current);
        }
        let text = |option: Option<&str>| match option {
            None => RichText::new("First announcing"),
            Some(source) if model.actions.sources().any(|known| known == source) => {
                RichText::new(outpost_name(source))
            }
            Some(source) => RichText::new(outpost_name(source)).weak(),
        };
        let mut chosen = None;
        egui::ComboBox::from_id_salt("actions outpost")
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
                "The outpost the actions are sent to, by default each to the first announcing it, preferring open connections",
            );
        if let Some(chosen) = chosen {
            self.outpost = chosen;
        }
    }

    /// Shows a checkbox whether the action at `path` has the readback its outpost
    /// announces, disabled if it announces none.
    fn readback_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>, path: &str) {
        let announced = self
            .announced(context.model, context.connected, path)
            .and_then(|action| action.readback.as_deref());
        let mut on = announced.is_some() && !self.readbacks_off.contains(path);
        let checkbox = egui::Checkbox::new(&mut on, announced.unwrap_or("none announced"));
        let response = ui
            .add_enabled(announced.is_some(), checkbox)
            .on_hover_text(
                "Whether the trace its outpost announces shows the state the action sets",
            )
            .on_disabled_hover_text("Its outpost announces no readback");
        if response.changed() {
            super::set_member(&mut self.readbacks_off, path, !on);
        }
    }

    /// Shows whether the actions at `paths` with several parameters are sent with
    /// their Send button or when one of their inputs changes.
    fn sending_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>, paths: &[String]) {
        let several: Vec<&str> = paths
            .iter()
            .filter_map(|path| self.announced(context.model, context.connected, path))
            .filter(|action| can_send_on_change(action))
            .map(|action| action.path.as_str())
            .collect();
        if several.is_empty() {
            return;
        }
        let names = short_names(several.iter().copied());
        properties_section(ui, "Sending", |ui| {
            for (path, name) in several.iter().zip(names) {
                let mut on_change = self.on_change.contains(*path);
                super::property_label(ui, name).on_hover_text(format!(
                    "{path}\nSent with its Send button, or each time one of its inputs changes, at most ten times a second"
                ));
                theme::segmented(
                    ui,
                    &mut on_change,
                    &[(false, "Send button"), (true, "On change")],
                );
                ui.end_row();
                super::set_member(&mut self.on_change, *path, on_change);
            }
        });
    }

    /// Shows whether the numbers with a minimum and a maximum of the actions at
    /// `paths` are entered with a slider or a field.
    fn number_inputs_ui(
        &mut self,
        ui: &mut egui::Ui,
        context: &PropertiesContext<'_>,
        paths: &[String],
    ) {
        let bounded: Vec<(&ActionDescriptor, &str)> = paths
            .iter()
            .filter_map(|path| self.announced(context.model, context.connected, path))
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
                super::property_label(ui, &label).on_hover_text(&action.path);
                theme::segmented(ui, &mut field, &[(false, "Slider"), (true, "Field")]);
                ui.end_row();
                super::set_member(&mut self.fields, &key, field);
            }
        });
    }
}

impl super::ViewInfo for Actions {
    const NAME: &'static str = "Actions";
    const ICON: &'static str = egui_phosphor::regular::SLIDERS_HORIZONTAL;
    const WRITES_VALUES: bool = false;
    const SHOWS_ACTIONS: bool = true;

    fn selectable(_trace: crate::app::model::traces::Trace<'_>) -> super::Selectable {
        super::Selectable::No("Action views show no traces".into())
    }
}

impl Viewable for Actions {
    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        let records = false;
        Some((&mut self.shown, records))
    }

    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let now = ui.input(|input| input.time);
        let model = context.model;
        let rows = self.rows(model, &context.invoker.connected);
        if rows.is_empty() {
            let text = if model.actions.is_empty() && context.invoker.connected.is_empty() {
                "Connect to an outpost to see its actions"
            } else if model.actions.is_empty() {
                "The outposts announced no actions"
            } else {
                "Check the actions to show in the tree on the left"
            };
            theme::hint(ui, text);
            return;
        }
        let names = short_names(rows.iter().map(|row| row.path));
        // why the connected actions are disabled, said once rather than in each hover
        let connected = rows
            .iter()
            .filter_map(|row| row.outpost)
            .any(|outpost| context.invoker.connected.contains(outpost));
        let hint = connected && !context.invoker.recording;
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::NONE.inner_margin(8.0).show(ui, |ui| {
                    if hint {
                        ui.weak(START_RECORDING);
                        ui.add_space(4.0);
                    }
                    // rows only as high as their content, the actions apart by a row of
                    // their own
                    egui::Grid::new(ui.id().with("actions"))
                        .num_columns(4)
                        .spacing([12.0, ROW_GAP])
                        .min_row_height(0.0)
                        .show(ui, |ui| {
                            for (index, (row, name)) in rows.iter().zip(&names).enumerate() {
                                if index > 0 {
                                    let gap = egui::vec2(0.0, ACTION_GAP);
                                    ui.allocate_exact_size(gap, egui::Sense::hover());
                                    ui.end_row();
                                }
                                let (Some(outpost), Some(action)) = (row.outpost, row.action)
                                else {
                                    Self::missing_ui(ui, row, name);
                                    continue;
                                };
                                let shown = Shown {
                                    id: ActionId::new(outpost, row.path),
                                    action,
                                    readback: self.readback(action),
                                    name,
                                };
                                self.action_ui(ui, context, &shown, now);
                            }
                        });
                });
            });
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        let paths: Vec<String> = self
            .rows(context.model, context.connected)
            .iter()
            .map(|row| row.path.to_owned())
            .collect();
        properties_section(ui, "Actions", |ui| {
            super::property_label(ui, "Outpost");
            self.outpost_ui(ui, context.model);
            ui.end_row();
            if paths.is_empty() {
                ui.label("");
                theme::panel_hint(ui, "Check the actions to show in the tree on the left");
                ui.end_row();
            }
        });
        if paths.is_empty() {
            return;
        }
        properties_section(ui, "Readbacks", |ui| {
            super::path_rows(ui, &paths, |ui, path| self.readback_ui(ui, context, path));
        });
        self.sending_ui(ui, context, &paths);
        self.number_inputs_ui(ui, context, &paths);
    }
}

#[cfg(test)]
mod tests;
