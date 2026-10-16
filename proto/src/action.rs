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
mod tests {
    use super::*;
    use crate::Message;
    use serde_cbor_2::Value as Cbor;

    fn text(text: &str) -> Cbor {
        Cbor::Text(text.to_owned())
    }

    fn map(entries: Vec<(&str, Cbor)>) -> Cbor {
        Cbor::Map(
            entries
                .into_iter()
                .map(|(key, value)| (text(key), value))
                .collect(),
        )
    }

    fn cbor(value: &impl Serialize) -> Cbor {
        serde_cbor_2::from_slice(&serde_cbor_2::to_vec(value).unwrap()).unwrap()
    }

    #[test]
    fn invocation_is_a_plain_cbor_map() {
        let request = Request::Invoke(Invocation {
            id: 7,
            path: "motor/speed".to_owned(),
            args: Data::Record(vec![("rpm".to_owned(), Data::Uint(1500))]),
        });
        let expected = map(vec![(
            "Invoke",
            map(vec![
                ("id", Cbor::Integer(7)),
                ("path", text("motor/speed")),
                ("args", map(vec![("rpm", Cbor::Integer(1500))])),
            ]),
        )]);
        assert_eq!(cbor(&request), expected);
    }

    #[test]
    fn results_are_ok_or_err() {
        let ok = Message::ActionResult(ActionResult {
            id: 1,
            result: Ok(()),
        });
        let err = Message::ActionResult(ActionResult {
            id: 2,
            result: Err("too fast".to_owned()),
        });
        let result = |id, result| {
            map(vec![(
                "ActionResult",
                map(vec![("id", Cbor::Integer(id)), ("result", result)]),
            )])
        };
        assert_eq!(cbor(&ok), result(1, map(vec![("Ok", Cbor::Null)])));
        assert_eq!(cbor(&err), result(2, map(vec![("Err", text("too fast"))])));
    }

    #[test]
    fn param_kinds_are_strings_or_maps() {
        assert_eq!(cbor(&ParamKind::Bool), text("Bool"));
        assert_eq!(cbor(&ParamKind::String), text("String"));
        assert_eq!(
            cbor(&ParamKind::Enum(vec!["a".to_owned()])),
            map(vec![("Enum", Cbor::Array(vec![text("a")]))])
        );
        assert_eq!(
            cbor(&ParamKind::Int {
                min: Some(-1),
                max: None
            }),
            map(vec![(
                "Int",
                map(vec![("min", Cbor::Integer(-1)), ("max", Cbor::Null)])
            )])
        );
    }

    #[test]
    fn descriptor_fields_may_be_left_out() {
        let bytes = serde_cbor_2::to_vec(&map(vec![(
            "Actions",
            Cbor::Array(vec![map(vec![
                ("path", text("reboot")),
                (
                    "params",
                    Cbor::Array(vec![map(vec![
                        ("name", text("delay")),
                        ("kind", map(vec![("Float", map(vec![]))])),
                    ])]),
                ),
            ])]),
        )]))
        .unwrap();
        let expected = Message::Actions(vec![ActionDescriptor {
            path: "reboot".to_owned(),
            description: String::new(),
            params: vec![Param {
                name: "delay".to_owned(),
                kind: ParamKind::Float {
                    min: None,
                    max: None,
                    step: None,
                },
                unit: String::new(),
                default: None,
            }],
            readback: None,
            confirm: false,
            enabled: true,
        }]);
        assert_eq!(
            serde_cbor_2::from_slice::<Message>(&bytes).unwrap(),
            expected
        );
    }
}
