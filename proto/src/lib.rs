use serde::{Deserialize, Serialize};

mod action;
mod data;

pub use action::{ActionDescriptor, ActionResult, Invocation, Param, ParamKind, Request};
pub use data::Data;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ValueMetadata {
    pub path: String,
    pub unit: String,
    // todo: enum values?
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Value {
    pub timestamp: u64,
    pub path: String,
    pub value: Data,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct LogRecord {
    pub timestamp: u64,
    pub level: u8,
    #[serde(default)]
    pub component: String,
    pub message: String,
}

/// A message from the target to blip.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Message {
    Value(Value),
    ValueMetadata(ValueMetadata),
    Log(LogRecord),
    /// All actions the target can carry out, replacing the ones announced before.
    Actions(Vec<ActionDescriptor>),
    ActionResult(ActionResult),
}
