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
use crate::app::theme;
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

/// Whether the action lists its parameters below its row, to be sent together.
fn lists_params(action: &ActionDescriptor) -> bool {
    action.params.len() > 1 || (action.confirm && !action.params.is_empty())
}

/// Whether the action can be chosen to be sent when one of its inputs changes instead
/// of with its Send button: one with several parameters that asks for no confirmation.
fn can_send_on_change(action: &ActionDescriptor) -> bool {
    action.params.len() > 1 && !action.confirm
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
        Samples::Strings(texts) => texts.get(last).map(Data::from),
        _ => None,
    }
}

/// The latest value of the trace at `path` as text with its unit.
fn latest_text(model: &Model, path: &str) -> Option<String> {
    let trace = model.traces.get(path)?;
    let last = trace.timestamps.len().checked_sub(1)?;
    let value = trace
        .samples
        .format(last, 4, Some(SHOWN_DIGITS), DisplayFormat::Default);
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
            let field = egui::TextEdit::singleline(&mut text)
                .margin(theme::FIELD_MARGIN)
                .desired_width(INPUT_WIDTH);
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
    let tokens = theme::tokens(ui.visuals());
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
    let mut text: String = ui.data(|data| data.get_temp(id)).unwrap_or(current);
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(id)
            .margin(theme::FIELD_MARGIN)
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
    let tokens = theme::tokens(ui.visuals());
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
        painter.rect_filled(rect, 1.0, theme::tokens(ui.visuals()).border);
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

        let listed = lists_params(action);
        let on_change = self.sends_on_change(action);
        let unsent = unsent(invoker, id, action, &values, readback_value.as_ref());
        // inputs that send only when told show what was not sent yet: those of a form
        // with a Send button, and a text field until Enter
        let single_text =
            !listed && action.params.first().map(|param| &param.kind) == Some(&ParamKind::String);
        let marks_unsent = (listed && !on_change) || single_text;
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
        ui.horizontal(|ui| {
            ui.label(*name).on_hover_text(&hover);
            if single_text && unsent[0] {
                unsent_dot(ui);
            }
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
                    if single_text && unsent[0] {
                        ui.weak("Enter to send");
                    }
                })
                .response
            } else {
                ui.label("")
            };
            if let Some(reason) = disabled {
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
            for ((param, value), unsent) in action.params.iter().zip(&mut values).zip(&unsent) {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.weak(&param.name);
                    if marks_unsent && *unsent {
                        unsent_dot(ui);
                    }
                });
                ui.add_enabled_ui(disabled.is_none(), |ui| {
                    let field = self.uses_field(&action.path, &param.name);
                    let changed = ui
                        .horizontal(|ui| input_ui(ui, &action.path, param, value, field, None))
                        .inner;
                    if on_change && changed != Change::None {
                        change = changed;
                    }
                    if !matches!(changed, Change::None | Change::Held) {
                        armed_at = None;
                    }
                });
                ui.label("");
                ui.label("");
                ui.end_row();
            }
            if !on_change {
                ui.label("");
                ui.add_enabled_ui(disabled.is_none(), |ui| {
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
                ui.label("");
                ui.label("");
                ui.end_row();
            }
        }

        let held = sends_dragged && matches!(change, Change::Held | Change::Dragging);
        ui.data_mut(|data| data.insert_temp(held_id, held));
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
        if disabled.is_some() || matches!(change, Change::None | Change::Held) {
            return;
        }
        let args: Args = action
            .params
            .iter()
            .map(|param| param.name.clone())
            .zip(values)
            .collect();
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
        ui.weak(name).on_hover_text(row.path);
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
            if on {
                self.readbacks_off.remove(path);
            } else {
                self.readbacks_off.insert(path.to_owned());
            }
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
                if on_change {
                    self.on_change.insert((*path).to_owned());
                } else {
                    self.on_change.remove(*path);
                }
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
                if field {
                    self.fields.insert(key);
                } else {
                    self.fields.remove(&key);
                }
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
                ui.weak("Check the actions to show in the tree on the left");
                ui.end_row();
            }
        });
        if paths.is_empty() {
            return;
        }
        let names = short_names(paths.iter().map(String::as_str));
        properties_section(ui, "Readbacks", |ui| {
            for (path, name) in paths.iter().zip(&names) {
                super::property_label(ui, name).on_hover_text(path);
                self.readback_ui(ui, context, path);
                ui.end_row();
            }
        });
        self.sending_ui(ui, context, &paths);
        self.number_inputs_ui(ui, context, &paths);
    }
}

#[cfg(test)]
mod tests {
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
        harness.state_mut().model.traces.append(&proto::Value {
            timestamp: 0,
            path: "on".to_owned(),
            value: Data::Bool(true),
        });
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
                    };
                    view.properties_ui(ui, &context);
                },
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
}
