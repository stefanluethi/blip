use serde::{Deserialize, Deserializer, Serialize};

/// What the timestamps of a target count from.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Clock {
    /// The Unix epoch, UTC.
    Epoch,
    /// The boot of the target.
    Boot,
}

/// The first message of a connection: who sends and what its timestamps mean.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Hello {
    /// The protocol version of the sender, see [`crate::VERSION`].
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "known_clock"
    )]
    pub clock: Option<Clock>,
    /// The software and version that sends, e.g. "sonde 1.2".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
}

/// A clock blip knows, `None` for one it does not, e.g. of a newer protocol.
fn known_clock<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Clock>, D::Error> {
    Ok(match Option::<String>::deserialize(d)?.as_deref() {
        Some("epoch") => Some(Clock::Epoch),
        Some("boot") => Some(Clock::Boot),
        _ => None,
    })
}
