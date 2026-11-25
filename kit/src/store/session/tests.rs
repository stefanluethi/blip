use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// A source made in the test: tells the events queued for it and keeps what it was
/// sent.
#[derive(Default, Clone)]
struct Scripted {
    events: Rc<RefCell<VecDeque<Event>>>,
    sent: Rc<RefCell<Vec<proto::Request>>>,
    closed: Rc<RefCell<bool>>,
}

impl Scripted {
    fn tell(&self, event: Event) {
        self.events.borrow_mut().push_back(event);
    }
}

impl Source for Scripted {
    fn poll(&mut self) -> Option<Event> {
        self.events.borrow_mut().pop_front()
    }

    fn send(&mut self, request: &proto::Request) {
        self.sent.borrow_mut().push(request.clone());
    }

    fn close(&mut self) {
        *self.closed.borrow_mut() = true;
    }
}

fn sample(timestamp: u64, path: &str, value: f64) -> proto::Message {
    proto::Message::Sample(proto::Sample {
        timestamp,
        path: path.to_owned(),
        value: proto::Data::Float(value),
    })
}

fn announce(path: &str) -> proto::Message {
    proto::Message::Actions(vec![proto::ActionDescriptor {
        path: path.to_owned(),
        description: String::new(),
        params: vec![],
        readback: None,
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }])
}

#[test]
fn a_source_of_the_application_feeds_the_store() {
    let source = Scripted::default();
    let mut session = Session::recording();
    session.add_source("sim", source.clone());
    assert!(session.open_sources().is_empty());

    source.tell(Event::Opened);
    source.tell(Event::message(&sample(1_000, "rpm", 1.0)));
    source.tell(Event::message(&sample(2_000, "rpm", 2.0)));
    session.update(0.0);

    assert_eq!(session.open_sources(), HashSet::from(["sim".to_owned()]));
    let series = session.store.traces.series("rpm").expect("the trace");
    assert_eq!(series.len(), 2);
    assert!(session.invoker.connected.contains("sim"));
}

#[test]
fn a_frame_may_hold_several_messages() {
    let source = Scripted::default();
    let mut session = Session::recording();
    session.add_source("sim", source.clone());
    let mut frame = serde_cbor_2::to_vec(&sample(1_000, "a", 1.0)).unwrap();
    frame.extend(serde_cbor_2::to_vec(&sample(2_000, "b", 2.0)).unwrap());
    source.tell(Event::Opened);
    source.tell(Event::Frame(frame));
    session.update(0.0);
    assert!(session.store.traces.series("a").is_some());
    assert!(session.store.traces.series("b").is_some());
}

#[test]
fn samples_are_kept_only_while_recording() {
    let source = Scripted::default();
    let mut session = Session::default();
    session.add_source("sim", source.clone());
    source.tell(Event::Opened);
    source.tell(Event::message(&sample(1_000, "rpm", 1.0)));
    session.update(0.0);
    assert!(!session.store.has_samples());

    // a recording starts only while connected
    assert!(!session.set_recording(true, false));
    assert!(!session.recording);
    assert!(session.set_recording(true, true));
    source.tell(Event::message(&sample(2_000, "rpm", 2.0)));
    session.update(0.1);
    assert!(session.store.has_samples());
}

#[test]
fn invocations_go_to_the_source_that_announced_the_action() {
    let source = Scripted::default();
    let other = Scripted::default();
    let mut session = Session::recording();
    session.add_source("sim", source.clone());
    session.add_source("other", other.clone());
    for source in [&source, &other] {
        source.tell(Event::Opened);
    }
    source.tell(Event::message(&announce("reset")));
    session.update(0.0);

    let action = crate::store::actions::ActionId::new("sim", "reset");
    session
        .invoker
        .invoke(&action, None, Default::default(), 0.0);
    session.send_invocations();
    assert_eq!(source.sent.borrow().len(), 1);
    assert!(other.sent.borrow().is_empty());

    // an action that is not announced is not sent
    let unknown = crate::store::actions::ActionId::new("sim", "unknown");
    session
        .invoker
        .invoke(&unknown, None, Default::default(), 0.0);
    session.send_invocations();
    assert_eq!(source.sent.borrow().len(), 1);
}

#[test]
fn a_source_that_ends_is_dropped_with_a_notice() {
    let source = Scripted::default();
    let mut session = Session::recording();
    session.add_source("sim", source.clone());
    source.tell(Event::Opened);
    source.tell(Event::Failed("unplugged".to_owned()));
    session.update(0.0);
    assert!(session.open_sources().is_empty());
    let notices = session.take_notices();
    assert_eq!(
        notices,
        [(
            Level::Error,
            "The connection to sim broke: unplugged".to_owned()
        )]
    );
    assert!(session.take_notices().is_empty());
}

#[test]
fn removing_a_source_closes_it() {
    let source = Scripted::default();
    let mut session = Session::default();
    session.add_source("sim", source.clone());
    session.remove_source("sim");
    assert!(*source.closed.borrow());
}

#[test]
fn the_first_source_to_open_starts_afresh_unless_recording() {
    let mut session = Session::default();
    session.receive(
        "old",
        &serde_cbor_2::to_vec(&announce("reset")).unwrap(),
        0.0,
    );
    assert!(!session.store.actions.is_empty());
    assert!(session.first_opened());
    assert!(session.store.actions.is_empty());

    session.recording = true;
    assert!(!session.first_opened());
}

#[test]
fn a_source_that_opens_after_the_last_one_ended_starts_afresh() {
    let first = Scripted::default();
    let second = Scripted::default();
    let mut session = Session::default();
    session.add_source("first", first.clone());
    session.add_source("second", second.clone());
    first.tell(Event::Opened);
    first.tell(Event::message(&announce("reset")));
    session.update(0.0);
    assert!(!session.store.actions.is_empty());

    // the first ends and the second opens in the same frame: none was open in between
    first.tell(Event::Closed);
    second.tell(Event::Opened);
    session.update(0.1);
    assert!(session.store.actions.is_empty());
    assert_eq!(session.open_sources(), HashSet::from(["second".to_owned()]));
    // a source that ended is closed before it is dropped
    assert!(*first.closed.borrow());
}

#[test]
fn a_source_of_the_application_delivers_traces_logs_and_actions() {
    let source = Scripted::default();
    let mut session = Session::recording();
    session.add_source("sim", source.clone());
    source.tell(Event::Opened);
    source.tell(Event::message(&sample(1_000, "rpm", 1.0)));
    source.tell(Event::message(&proto::Message::Log(proto::LogRecord {
        timestamp: 2_000,
        path: "log/sim".to_owned(),
        level: proto::level::WARN,
        message: "hot".to_owned(),
    })));
    source.tell(Event::message(&announce("reset")));
    session.update(0.0);

    assert!(session.store.traces.series("rpm").is_some());
    let logs = session.store.logs.get("log/sim").expect("the log path");
    assert_eq!(logs.between(0, u64::MAX).len(), 1);
    assert!(session.store.actions.announced("sim", "reset").is_some());
}

#[test]
fn the_progress_and_the_answer_of_an_invocation_are_readable() {
    use crate::store::actions::{ActionId, Outcome};
    let source = Scripted::default();
    let mut session = Session::recording();
    session.add_source("sim", source.clone());
    source.tell(Event::Opened);
    source.tell(Event::message(&announce("reset")));
    session.update(0.0);

    // invoked from anywhere in the application, with no action widget shown
    let action = ActionId::new("sim", "reset");
    session
        .invoker
        .invoke(&action, None, Default::default(), 0.0);
    assert_eq!(
        session.invoker.outcome(&action, 0.0),
        Some(&Outcome::Pending)
    );
    session.send_invocations();
    let sent = source.sent.borrow().clone();
    let [proto::Request::Invoke(invocation)] = sent.as_slice() else {
        panic!("one invocation, got {sent:?}");
    };

    // the answer of the source ends the wait
    source.tell(Event::message(&proto::Message::ActionResult(
        proto::ActionResult {
            id: invocation.id,
            result: Err("jammed".to_owned()),
        },
    )));
    session.update(0.1);
    let outcome = session.invoker.outcome(&action, 0.1).expect("an outcome");
    assert_ne!(outcome, &Outcome::Pending);
    assert!(format!("{outcome:?}").contains("jammed"), "{outcome:?}");
}

#[test]
fn recorded_samples_are_unsaved_until_saved_cleared_or_replaced() {
    let message = serde_cbor_2::to_vec(&sample(1_000, "rpm", 1.0)).unwrap();
    let mut session = Session::default();
    // samples dropped while not recording are nothing to save
    session.receive("sim", &message, 0.0);
    assert!(!session.is_unsaved());

    session.recording = true;
    session.receive("sim", &message, 0.0);
    assert!(session.is_unsaved());
    session.mark_saved();
    assert!(!session.is_unsaved());
    // more recorded after the save is unsaved again
    session.receive("sim", &message, 0.0);
    assert!(session.is_unsaved());
    // stopping keeps it unsaved
    session.set_recording(false, true);
    assert!(session.is_unsaved());

    session.clear_data();
    assert!(!session.is_unsaved());
    session.recording = true;
    session.receive("sim", &message, 0.0);
    session.start_fresh(MemoryStore::default());
    assert!(!session.is_unsaved());
}

#[test]
fn the_first_source_to_open_keeps_an_unsaved_recording() {
    let message = serde_cbor_2::to_vec(&sample(1_000, "rpm", 1.0)).unwrap();
    let mut session = Session::default();
    session.set_recording(true, true);
    session.receive("old", &message, 0.0);
    session.set_recording(false, false);
    assert!(!session.first_opened());
    assert!(session.store.has_samples());

    // once saved, it starts afresh
    session.mark_saved();
    assert!(session.first_opened());
    assert!(!session.store.has_samples());
}

#[test]
fn a_session_waits_for_record_while_connected_without_samples() {
    let source = Scripted::default();
    let mut session = Session::default();
    assert!(!session.waiting_for_record());
    session.add_source("sim", source.clone());
    source.tell(Event::Opened);
    source.tell(Event::message(&sample(1_000, "rpm", 1.0)));
    session.update(0.0);
    assert!(session.waiting_for_record());

    session.set_recording(true, true);
    assert!(!session.waiting_for_record());
    source.tell(Event::message(&sample(2_000, "rpm", 2.0)));
    session.update(0.1);
    // stopped, the recorded samples are shown
    session.set_recording(false, true);
    assert!(!session.waiting_for_record());
}

#[test]
fn a_kept_recording_takes_no_logs_of_a_new_connection() {
    let encode = |message: &proto::Message| serde_cbor_2::to_vec(message).unwrap();
    let mut session = Session::default();
    session.set_recording(true, true);
    session.receive("old", &encode(&sample(1_000, "rpm", 1.0)), 0.0);
    session.set_recording(false, false);
    assert!(!session.first_opened());
    let log = proto::Message::Log(proto::LogRecord {
        timestamp: 9_000_000_000,
        path: "log/sim".to_owned(),
        level: proto::level::WARN,
        message: "hot".to_owned(),
    });
    session.receive("new", &encode(&log), 0.0);
    assert!(session.store.logs.get("log/sim").is_none());
    assert_eq!(session.store.time_span(), Some((1_000, 1_000)));
}
