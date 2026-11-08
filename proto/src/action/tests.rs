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
        disabled_reason: None,
    }]);
    assert_eq!(
        serde_cbor_2::from_slice::<Message>(&bytes).unwrap(),
        expected
    );
}
