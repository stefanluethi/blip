use super::*;

fn items_of(value: &impl AsValue) -> Value {
    let mut v = Value::default();
    v.set(value);
    v
}

#[test]
fn an_item_takes_16_bytes() {
    assert_eq!(std::mem::size_of::<Item>(), 16);
}

fn sample_record() -> impl AsValue {
    record(|r| {
        r.field("t", 21.5);
        r.field("acc", [0.1, 0.2]);
        r.field("mode", "run");
    })
}

#[test]
fn the_tape_holds_the_items_in_the_order_they_are_written() {
    let value = items_of(&sample_record());
    assert_eq!(
        value.items,
        vec![
            Item::Record { len: 3, size: 8 },
            Item::Key { start: 0, len: 1 },
            Item::Float(21.5),
            Item::Key { start: 1, len: 3 },
            Item::Array { len: 2, size: 2 },
            Item::Float(0.1),
            Item::Float(0.2),
            Item::Key { start: 4, len: 4 },
            Item::Text { start: 8, len: 3 },
        ]
    );
    assert_eq!(value.texts, "taccmoderun");
}

fn node_of(value: impl AsValue) -> Value {
    let mut v = Value::default();
    v.set(value);
    v
}

#[test]
fn values_of_the_application_are_written_without_proto() {
    assert!(matches!(node_of(3.2).as_ref().node(), Node::Float(f) if f == 3.2));
    assert!(matches!(node_of(-3i32).as_ref().node(), Node::Int(-3)));
    assert!(matches!(node_of(7u8).as_ref().node(), Node::Uint(7)));
    assert!(matches!(node_of(5i64).as_ref().node(), Node::Uint(5)));
    assert!(matches!(node_of(None::<f64>).as_ref().node(), Node::Null));
    assert!(matches!(node_of(Some(2.0f32)).as_ref().node(), Node::Float(f) if f == 2.0));
    let array = node_of([1u16, 2]);
    let Node::Array(elements) = array.as_ref().node() else {
        panic!("not an array");
    };
    let nodes: Vec<_> = elements.map(|e| e.to_data()).collect();
    assert_eq!(nodes, vec![Data::Uint(1), Data::Uint(2)]);
    assert!(matches!(
        node_of("x".to_owned()).as_ref().node(),
        Node::Text("x")
    ));
    let bools = node_of(vec![true]);
    assert_eq!(
        bools.as_ref().to_data(),
        Data::Array(vec![Data::Bool(true)])
    );
}

struct Nothing;
impl AsValue for Nothing {
    fn write(&self, _: &mut Writer<'_>) {}
}

struct Two;
impl AsValue for Two {
    fn write(&self, to: &mut Writer<'_>) {
        to.uint(1);
        to.uint(2);
    }
}

#[test]
fn an_as_value_of_no_value_is_null_and_of_more_keeps_the_first() {
    assert!(matches!(node_of(Nothing).as_ref().node(), Node::Null));
    let two = node_of(Two);
    assert!(matches!(two.as_ref().node(), Node::Uint(1)));
    assert_eq!(two.items.len(), 1);
}

#[test]
fn values_survive_the_way_through_data() {
    let f = Data::Float;
    let cases = vec![
        Data::Null,
        Data::Uint(u64::MAX),
        Data::Int(i64::MIN),
        Data::Float(-0.0),
        Data::Bool(false),
        Data::String("ä".into()),
        Data::Array(vec![]),
        Data::Array(vec![
            Data::Array(vec![f(1.0), f(2.0)]),
            Data::Null,
            Data::Array(vec![f(3.0), f(4.0)]),
        ]),
        Data::Record(vec![(
            "a".into(),
            Data::Record(vec![("b".into(), Data::Uint(1))]),
        )]),
        Data::Array(vec![
            Data::Record(vec![("a".into(), Data::Uint(1))]),
            Data::Record(vec![("a".into(), Data::Uint(2))]),
        ]),
        Data::Array(vec![Data::String("a".into()), Data::String("b".into())]),
        Data::Record(vec![]),
    ];
    for d in cases {
        let back = Value::from(&d).as_ref().to_data();
        if let (Data::Float(a), Data::Float(b)) = (&d, &back) {
            assert_eq!(a.to_bits(), b.to_bits());
        } else {
            assert_eq!(back, d);
        }
    }
    assert_eq!(Value::from(&Data::Int(5)).as_ref().to_data(), Data::Uint(5));
    let Data::Float(nan) = Value::from(&Data::Float(f64::NAN)).as_ref().to_data() else {
        panic!("not a float");
    };
    assert!(nan.is_nan());
}

#[test]
fn nodes_walk_arrays_and_records() {
    let value = items_of(&sample_record());
    let Node::Record(fields) = value.as_ref().node() else {
        panic!("not a record");
    };
    let fields: Vec<_> = fields.collect();
    let names: Vec<_> = fields.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, ["t", "acc", "mode"]);
    let Node::Array(elements) = fields[1].1.node() else {
        panic!("not an array");
    };
    assert_eq!(elements.size_hint(), (2, Some(2)));
    assert_eq!(elements.count(), 2);
    assert!(matches!(fields[2].1.node(), Node::Text("run")));
}

#[test]
fn rows_hold_their_values_row_by_row() {
    let mut buf = RowsBuf::default();
    buf.push_with(|to| to.value("a"));
    buf.push_with(|to| to.value([1.0, 2.0]));
    buf.push_with(|to| to.value("bc"));
    let rows = buf.rows();
    assert_eq!(rows.len(), 3);
    assert!(!rows.is_empty());
    assert!(matches!(rows.get(2).unwrap().node(), Node::Text("bc")));
    assert!(matches!(rows.last().unwrap().node(), Node::Text("bc")));
    let data: Vec<Data> = rows.iter().map(ValueRef::to_data).collect();
    assert_eq!(
        data,
        [
            Data::from("a"),
            Data::Array(vec![Data::Float(1.0), Data::Float(2.0)]),
            Data::from("bc"),
        ]
    );
    assert!(rows.get(3).is_none());
    buf.clear();
    assert!(buf.rows().is_empty());
    assert!(buf.rows().last().is_none());
}

#[test]
fn a_value_is_copied_without_losing_its_texts() {
    let from = items_of(&sample_record());
    let mut to = items_of(&"something else");
    to.copy_from(from.as_ref());
    assert_eq!(to, from);
}
