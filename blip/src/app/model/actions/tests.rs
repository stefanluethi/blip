use super::*;

fn args(rpm: u64) -> Args {
    vec![("rpm".to_owned(), Data::Uint(rpm))]
}

/// The ids, connections, paths and arguments of the invocations sent since the
/// last call.
fn sent(invoker: &mut Invoker) -> Vec<(u32, String, String, Data)> {
    invoker
        .take_outbox()
        .into_iter()
        .map(|(source, Request::Invoke(invocation))| {
            (invocation.id, source, invocation.path, invocation.args)
        })
        .collect()
}

fn answer(id: u32, result: Result<(), &str>) -> ActionResult {
    ActionResult {
        id,
        result: result.map_err(str::to_owned),
    }
}

fn descriptor(path: &str, readback: Option<&str>) -> ActionDescriptor {
    ActionDescriptor {
        path: path.to_owned(),
        description: String::new(),
        params: vec![],
        readback: readback.map(str::to_owned),
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }
}

/// The action at `path` of the connection without a name.
fn unnamed(path: &str) -> ActionId {
    ActionId::new("", path)
}

/// A model that received `samples` values of the trace "on", the readback of
/// "enable", while not recording.
fn model(samples: u64) -> Model {
    let mut model = Model::default();
    model
        .actions
        .set("", vec![descriptor("enable", Some("on"))]);
    for timestamp in 0..samples {
        let value = proto::Message::Value(proto::Value {
            timestamp,
            path: "on".to_owned(),
            value: Data::Bool(true),
        });
        model.receive_unrecorded("", &serde_cbor_2::to_vec(&value).unwrap());
    }
    model
}

#[test]
fn invocations_are_sent_with_new_ids_and_answered() {
    let mut invoker = Invoker::default();
    let (speed, reboot) = (unnamed("speed"), ActionId::new("b", "reboot"));
    invoker.invoke(&speed, None, args(1), 0.0);
    invoker.invoke(&reboot, None, vec![], 0.0);
    assert_eq!(
        sent(&mut invoker),
        [
            (0, String::new(), "speed".to_owned(), Data::Record(args(1))),
            (1, "b".to_owned(), "reboot".to_owned(), Data::Record(vec![]))
        ]
    );
    assert_eq!(invoker.outcome(&speed, 0.0), Some(&Outcome::Pending));

    invoker.receive("", &answer(0, Ok(())), 1.0, &model(0));
    invoker.receive("b", &answer(1, Err("busy")), 1.0, &model(0));
    assert_eq!(invoker.outcome(&speed, 2.0), Some(&Outcome::Accepted(1.0)));
    assert_eq!(invoker.outcome(&speed, 1.0 + SHOW_ACCEPTED + 0.1), None);
    assert_eq!(
        invoker.outcome(&reboot, 100.0),
        Some(&Outcome::Rejected("busy".to_owned()))
    );
    // unknown and repeated answers change nothing
    invoker.receive("b", &answer(1, Ok(())), 2.0, &model(0));
    invoker.receive("", &answer(7, Ok(())), 2.0, &model(0));
    assert_eq!(
        invoker.outcome(&reboot, 100.0),
        Some(&Outcome::Rejected("busy".to_owned()))
    );
}

#[test]
fn only_the_latest_invocation_of_an_action_shows() {
    let mut invoker = Invoker::default();
    let speed = unnamed("speed");
    invoker.invoke(&speed, None, args(1), 0.0);
    invoker.invoke(&speed, None, args(2), 0.0);
    invoker.receive("", &answer(0, Err("old")), 0.5, &model(0));
    assert_eq!(invoker.outcome(&speed, 0.5), Some(&Outcome::Pending));
    invoker.receive("", &answer(1, Ok(())), 0.6, &model(0));
    assert_eq!(invoker.outcome(&speed, 0.6), Some(&Outcome::Accepted(0.6)));
}

#[test]
fn the_same_path_of_several_outposts_has_its_own_outcomes() {
    let mut invoker = Invoker::default();
    let (a, b) = (ActionId::new("a", "speed"), ActionId::new("b", "speed"));
    invoker.invoke(&a, None, args(1), 0.0);
    invoker.invoke(&b, None, args(2), 0.0);
    invoker.receive("b", &answer(1, Err("no")), 0.1, &model(0));
    assert_eq!(invoker.outcome(&a, 0.1), Some(&Outcome::Pending));
    assert_eq!(
        invoker.outcome(&b, 0.1),
        Some(&Outcome::Rejected("no".to_owned()))
    );
}

#[test]
fn inputs_follow_the_readback_once_it_is_newer_than_the_answer() {
    let mut invoker = Invoker::default();
    let enable = unnamed("enable");
    assert!(invoker.follows_readback(&enable, 0));
    invoker.invoke(&enable, Some("on"), vec![], 0.0);
    assert!(!invoker.follows_readback(&enable, 10));
    // accepted when the readback had 10 samples, which show the state before
    invoker.receive("", &answer(0, Ok(())), 0.1, &model(10));
    assert!(!invoker.follows_readback(&enable, 10));
    assert!(invoker.follows_readback(&enable, 11));
    // a rejected invocation changed nothing, the readback is right
    invoker.invoke(&enable, Some("on"), vec![], 1.0);
    invoker.receive("", &answer(1, Err("no")), 1.1, &model(20));
    assert!(invoker.follows_readback(&enable, 11));
}

#[test]
fn answers_count_only_from_the_connection_of_the_invocation() {
    let mut invoker = Invoker::default();
    let speed = unnamed("speed");
    invoker.invoke(&speed, None, args(1), 0.0);
    invoker.receive("other", &answer(0, Err("not mine")), 0.1, &model(0));
    assert_eq!(invoker.outcome(&speed, 0.1), Some(&Outcome::Pending));
    invoker.receive("", &answer(0, Ok(())), 0.2, &model(0));
    assert_eq!(invoker.outcome(&speed, 0.2), Some(&Outcome::Accepted(0.2)));
}

#[test]
fn actions_are_kept_per_connection_with_their_paths() {
    let mut actions = Actions::default();
    actions.set("a", vec![descriptor("speed", Some("rpm"))]);
    actions.set(
        "b",
        vec![descriptor("speed", None), descriptor("park", None)],
    );
    // announcing again replaces the actions of the connection in their place
    actions.set(
        "a",
        vec![descriptor("speed", Some("rpm")), descriptor("stop", None)],
    );
    assert_eq!(
        actions.paths().collect::<Vec<_>>(),
        ["speed", "stop", "park"]
    );
    let sources: Vec<&str> = actions
        .announcers("speed")
        .map(|action| action.source.as_str())
        .collect();
    assert_eq!(sources, ["a", "b"]);
    assert_eq!(actions.sources().collect::<Vec<_>>(), ["a", "b"]);
    let readback = |source| actions.announced(source, "speed").unwrap().readback.clone();
    assert_eq!(readback("a").as_deref(), Some("rpm"));
    assert_eq!(readback("b"), None);
    assert!(actions.announced("b", "stop").is_none());
}

#[test]
fn unanswered_invocations_time_out_and_still_take_a_late_answer() {
    let mut invoker = Invoker::default();
    let speed = unnamed("speed");
    invoker.invoke(&speed, None, args(1), 0.0);
    assert_eq!(invoker.repaint_after(0.0), Some(DRAG_INTERVAL));
    invoker.update(ANSWER_TIMEOUT);
    assert_eq!(invoker.outcome(&speed, 0.0), Some(&Outcome::Pending));
    invoker.update(ANSWER_TIMEOUT + 0.1);
    assert_eq!(invoker.outcome(&speed, 0.0), Some(&Outcome::Unanswered));
    // the seconds without an answer are counted
    assert_eq!(invoker.repaint_after(ANSWER_TIMEOUT + 0.1), Some(1.0));
    // a late answer counts, as the outpost answers once the action returns
    invoker.receive("", &answer(0, Ok(())), 8.0, &model(0));
    assert_eq!(invoker.outcome(&speed, 8.0), Some(&Outcome::Accepted(8.0)));
    assert_eq!(invoker.accepted(&speed), Some(&args(1)));
    assert_eq!(invoker.repaint_after(8.0 + SHOW_ACCEPTED + 0.1), None);
}

#[test]
fn only_the_latest_invocation_waits_for_a_late_answer() {
    let mut invoker = Invoker::default();
    let speed = unnamed("speed");
    invoker.invoke(&speed, None, args(1), 0.0);
    invoker.invoke(&speed, None, args(2), 0.0);
    invoker.update(ANSWER_TIMEOUT + 0.1);
    assert_eq!(invoker.pending.len(), 1);
    invoker.receive("", &answer(1, Err("no")), 9.0, &model(0));
    assert_eq!(
        invoker.outcome(&speed, 9.0),
        Some(&Outcome::Rejected("no".to_owned()))
    );
}

#[test]
fn rejections_are_taken_once_and_accepted_arguments_kept() {
    let mut invoker = Invoker::default();
    let speed = unnamed("speed");
    invoker.invoke(&speed, None, args(1), 0.0);
    invoker.receive("", &answer(0, Ok(())), 0.1, &model(0));
    invoker.invoke(&speed, None, args(2), 1.0);
    invoker.receive("", &answer(1, Err("too fast")), 1.1, &model(0));
    assert_eq!(invoker.latest(&speed).unwrap().args, args(2));
    assert_eq!(invoker.accepted(&speed), Some(&args(1)));
    assert!(invoker.take_rejected(&speed));
    assert!(!invoker.take_rejected(&speed));
    // a new invocation drops a rejection not taken yet
    invoker.invoke(&speed, None, args(3), 2.0);
    invoker.receive("", &answer(2, Err("too fast")), 2.1, &model(0));
    invoker.invoke(&speed, None, args(4), 3.0);
    assert!(!invoker.take_rejected(&speed));
}

#[test]
fn dragging_sends_at_most_every_interval_and_the_last_value() {
    let mut invoker = Invoker::default();
    let speed = unnamed("speed");
    invoker.invoke_dragging(&speed, None, args(1), 0.0);
    invoker.invoke_dragging(&speed, None, args(2), 0.03);
    invoker.invoke_dragging(&speed, None, args(3), 0.06);
    invoker.update(0.08);
    let values = |sent: Vec<(u32, String, String, Data)>| -> Vec<Data> {
        sent.into_iter().map(|(_, _, _, args)| args).collect()
    };
    assert_eq!(values(sent(&mut invoker)), [Data::Record(args(1))]);
    assert_eq!(invoker.repaint_after(0.08), Some(DRAG_INTERVAL));
    // the held back value is sent once the interval passed
    invoker.update(0.1);
    assert_eq!(values(sent(&mut invoker)), [Data::Record(args(3))]);
    invoker.update(0.5);
    assert!(sent(&mut invoker).is_empty());

    // releasing sends the final value right away, dropping held back ones
    invoker.invoke_dragging(&speed, None, args(4), 0.5);
    invoker.invoke_dragging(&speed, None, args(5), 0.52);
    invoker.finish_dragging(&speed, None, args(6), 0.55);
    invoker.update(1.0);
    assert_eq!(
        values(sent(&mut invoker)),
        [Data::Record(args(4)), Data::Record(args(6))]
    );

    // releasing at the value sent last, or without dragging, sends nothing
    invoker.invoke_dragging(&speed, None, args(7), 2.0);
    invoker.finish_dragging(&speed, None, args(7), 2.05);
    invoker.finish_dragging(&speed, None, args(8), 3.0);
    assert_eq!(values(sent(&mut invoker)), [Data::Record(args(7))]);
}

#[test]
fn held_back_values_keep_the_readback_of_their_action() {
    let mut invoker = Invoker::default();
    let enable = unnamed("enable");
    invoker.invoke_dragging(&enable, Some("on"), args(1), 0.0);
    invoker.invoke_dragging(&enable, Some("on"), args(2), 0.05);
    invoker.update(0.1);
    assert_eq!(sent(&mut invoker).len(), 2);
    // the held back invocation is accepted when the readback had 3 samples
    invoker.receive("", &answer(1, Ok(())), 0.2, &model(3));
    assert!(!invoker.follows_readback(&enable, 3));
    assert!(invoker.follows_readback(&enable, 4));
}

#[test]
fn resetting_keeps_the_inputs() {
    let mut invoker = Invoker::default();
    invoker
        .inputs
        .insert((unnamed("speed"), "rpm".to_owned()), Data::Uint(5));
    let speed = unnamed("speed");
    invoker.invoke(&speed, None, args(1), 0.0);
    invoker.reset();
    assert!(invoker.take_outbox().is_empty());
    assert_eq!(invoker.outcome(&speed, 0.0), None);
    assert_eq!(invoker.inputs.len(), 1);
}
