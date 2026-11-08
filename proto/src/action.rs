//! Actions the target announces and blip invokes, e.g. to set a motor setpoint or a log
//! level.

use crate::Data;
use serde::{Deserialize, Serialize};

/// An action the target can carry out, as announced in [`crate::Message::Actions`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ActionDescriptor {
    /// Names the action, levels separated by "/" like the paths of traces.
    pub path: String,
    #[serde(default)]
    pub description: String,
    /// The parameters in the order shown, none for an action that only needs a button.
    #[serde(default)]
    pub params: Vec<Param>,
    /// The path of the trace that shows the current value of what the action sets, e.g.
    /// the measured speed of a speed setpoint.
    #[serde(default)]
    pub readback: Option<String>,
    /// Whether the user confirms the action before it is sent, e.g. for a reboot.
    #[serde(default)]
    pub confirm: bool,
    /// Whether the target can carry out the action now, e.g. not while the link to the
    /// device is down.
    #[serde(default = "enabled")]
    pub enabled: bool,
    /// Why the target cannot carry out the action now, if it says, shown while it is
    /// not `enabled`, e.g. "calibrating, keep the device still".
    #[serde(default)]
    pub disabled_reason: Option<String>,
}

fn enabled() -> bool {
    true
}

/// A parameter of an action.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Param {
    /// The name of the argument in [`Invocation::args`].
    pub name: String,
    pub kind: ParamKind,
    #[serde(default)]
    pub unit: String,
    /// The value shown before the user enters one, if there is no readback value.
    #[serde(default)]
    pub default: Option<Data>,
}

/// The values a parameter takes, and so how blip lets the user enter it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ParamKind {
    Bool,
    /// An integer within the bounds, if any.
    Int {
        #[serde(default)]
        min: Option<i64>,
        #[serde(default)]
        max: Option<i64>,
    },
    /// A float within the bounds, if any, in steps of `step` if given.
    Float {
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        step: Option<f64>,
    },
    /// One of these strings.
    Enum(Vec<String>),
    String,
}

/// A message from blip to the target.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Request {
    Invoke(Invocation),
}

/// Asks the target to carry out an action, answered by an [`ActionResult`] of the same
/// id.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Invocation {
    /// Chosen by blip, unique among the invocations of a connection.
    pub id: u32,
    /// The path of the action.
    pub path: String,
    /// A record of the arguments by the names of the parameters.
    pub args: Data,
}

/// Whether the target carried out an invocation, sent to the client that invoked it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ActionResult {
    /// The id of the invocation.
    pub id: u32,
    /// Why the target rejected the invocation, if it did.
    pub result: Result<(), String>,
}

#[cfg(test)]
mod tests;
