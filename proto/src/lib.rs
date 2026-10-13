use serde::{Deserialize, Serialize};

mod data;

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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Message {
    Value(Value),
    ValueMetadata(ValueMetadata),
    Log(LogRecord),
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Action {
    SetMotorSpeed(u16),
}
