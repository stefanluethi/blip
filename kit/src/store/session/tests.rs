use super::*;
use crate::store::actions::Actions;
use crate::store::logs::Logs;
use crate::store::numbers::Labels;
use crate::store::recording::read_file;
use crate::store::time_base::TimeBase;
use crate::store::traces::{Series, Trace};
use crate::store::value::record;
use crate::store::{Input, Loaded, Times, Window};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// What a [`Scripted`] source hands to its inbox.
enum Told {
    Opened,
    Frame(Vec<u8>),
    Message(proto::Message),
    Closed,
    Failed(String),
}

/// A source made in the test: tells what is queued for it and keeps what it was sent.
#[derive(Default, Clone)]
struct Scripted {
    events: Rc<RefCell<VecDeque<Told>>>,
    sent: Rc<RefCell<Vec<proto::Request>>>,
    closed: Rc<RefCell<bool>>,
}

impl Scripted {
    fn tell(&self, event: Told) {
        self.events.borrow_mut().push_back(event);
    }
}

impl Source for Scripted {
    fn poll(&mut self, inbox: &mut Inbox<'_>) {
        while let Some(told) = self.events.borrow_mut().pop_front() {
            match told {
                Told::Opened => inbox.opened(),
                Told::Frame(frame) => inbox.frame(&frame),
                Told::Message(message) => inbox.message(&message),
                Told::Closed => inbox.closed(),
                Told::Failed(reason) => inbox.failed(reason),
            }
        }
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

    source.tell(Told::Opened);
    source.tell(Told::Message(sample(1_000, "rpm", 1.0)));
    source.tell(Told::Message(sample(2_000, "rpm", 2.0)));
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
    source.tell(Told::Opened);
    source.tell(Told::Frame(frame));
    session.update(0.0);
    assert!(session.store.traces.series("a").is_some());
    assert!(session.store.traces.series("b").is_some());
}

#[test]
fn samples_are_kept_only_while_recording() {
    let source = Scripted::default();
    let mut session = Session::default();
    session.add_source("sim", source.clone());
    source.tell(Told::Opened);
    source.tell(Told::Message(sample(1_000, "rpm", 1.0)));
    session.update(0.0);
    assert!(!session.store.has_samples());

    // a recording starts only while connected
    assert!(!session.set_recording(true, false));
    assert!(!session.recording);
    assert!(session.set_recording(true, true));
    source.tell(Told::Message(sample(2_000, "rpm", 2.0)));
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
        source.tell(Told::Opened);
    }
    source.tell(Told::Message(announce("reset")));
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
    source.tell(Told::Opened);
    source.tell(Told::Failed("unplugged".to_owned()));
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
    session.inbox("old", 0.0).message(&announce("reset"));
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
    first.tell(Told::Opened);
    first.tell(Told::Message(announce("reset")));
    session.update(0.0);
    assert!(!session.store.actions.is_empty());

    // the first ends and the second opens in the same frame: none was open in between
    first.tell(Told::Closed);
    second.tell(Told::Opened);
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
    source.tell(Told::Opened);
    source.tell(Told::Message(sample(1_000, "rpm", 1.0)));
    source.tell(Told::Message(proto::Message::Log(proto::LogRecord {
        timestamp: 2_000,
        path: "log/sim".to_owned(),
        level: proto::level::WARN,
        message: "hot".to_owned(),
    })));
    source.tell(Told::Message(announce("reset")));
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
    source.tell(Told::Opened);
    source.tell(Told::Message(announce("reset")));
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
    source.tell(Told::Message(proto::Message::ActionResult(
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
fn typed_calls_reach_the_store_and_the_recording() {
    let mut session = Session::recording();
    let mut inbox = session.inbox("sim", 0.0);
    inbox.sample(1, "a", 3.2);
    inbox.samples(
        "b",
        Times::Period {
            timestamp: 2,
            period: 1,
        },
        [1u64, 2],
    );
    inbox.log(3, "", 2, "hi");
    assert_eq!(inbox.finish().messages, 3);
    assert_eq!(
        session.store.traces.get("a").expect("a").timestamps.len(),
        1
    );
    assert_eq!(
        session.store.traces.get("b").expect("b").timestamps.len(),
        2
    );
    assert_eq!(session.store.logs.entries().count(), 1);
    assert_eq!(session.recorder.messages(), 3);

    let mut session = Session::default();
    let mut inbox = session.inbox("sim", 0.0);
    inbox.actions(&[proto::ActionDescriptor {
        path: "set".to_owned(),
        description: String::new(),
        params: vec![],
        readback: Some("a".to_owned()),
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }]);
    inbox.sample(1, "a", 3.2);
    assert!(session.store.traces.get("a").is_none());
    assert!(session.store.readback("a").is_some());
    assert_eq!(session.recorder.messages(), 0);
}

#[test]
fn a_block_that_does_not_fit_is_cut_to_its_rows() {
    let mut session = Session::recording();
    let mut inbox = session.inbox("sim", 0.0);
    inbox.samples("a", Times::Each(&[1, 2, 3]), [1.0, 2.0]);
    inbox.samples("b", Times::Each(&[1]), [1.0, 2.0]);
    assert_eq!(
        session.store.traces.get("a").expect("a").timestamps.len(),
        2
    );
    assert_eq!(
        session.store.traces.get("b").expect("b").timestamps.len(),
        1
    );
}

#[test]
fn a_block_of_endless_values_ends_with_its_timestamps() {
    let mut session = Session::recording();
    let mut inbox = session.inbox("sim", 0.0);
    inbox.samples("adc", Times::Each(&[10, 20]), std::iter::repeat(1.5));
    let trace = session.store.traces.get("adc").expect("adc");
    assert_eq!(trace.timestamps.len(), 2);
    assert_eq!(trace.timestamps[0], 10);
    assert_eq!(trace.timestamps[1], 20);
}

#[test]
fn an_ended_inbox_ignores_calls_and_frames_count() {
    let mut session = Session::recording();
    let mut inbox = session.inbox("sim", 0.0);
    inbox.hello(&proto::Hello::default());
    inbox.sample(1, "a", 1.0);
    inbox.closed();
    inbox.sample(2, "a", 2.0);
    inbox.failed("late");
    let polled = inbox.finish();
    assert_eq!(polled.messages, 2);
    assert_eq!(
        polled.events,
        [
            SourceEvent::Hello(proto::Hello::default()),
            SourceEvent::Closed
        ]
    );
    assert_eq!(
        session.store.traces.get("a").expect("a").timestamps.len(),
        1
    );
}

/// What a [`Recorded`] store was given, as owned values.
#[derive(Debug, PartialEq)]
enum Got {
    Sample {
        source: String,
        timestamp: u64,
        path: String,
        value: proto::Data,
    },
    Samples {
        source: String,
        path: String,
        times: proto::SampleTimes,
        values: Vec<proto::Data>,
    },
    Latest {
        source: String,
        timestamp: u64,
        path: String,
        value: proto::Data,
    },
    Log {
        source: String,
        timestamp: u64,
        path: String,
        level: u8,
        message: String,
    },
    Metadata {
        source: String,
        metadata: proto::TraceMetadata,
    },
    Actions {
        source: String,
        actions: Vec<proto::ActionDescriptor>,
    },
    Clock {
        source: String,
        clock: Option<TimeBase>,
    },
}

/// A store that is not the in-memory one: it keeps what it is given as owned values and
/// forwards everything to a [`MemoryStore`], as the store of an application would.
#[derive(Default)]
struct Recorded {
    got: Vec<Got>,
    memory: MemoryStore,
}

impl Ingest for Recorded {
    fn ingest(&mut self, source: &str, input: Input<'_>) {
        let source_name = source.to_owned();
        self.got.push(match input {
            Input::Sample {
                timestamp,
                path,
                value,
            } => Got::Sample {
                source: source_name,
                timestamp,
                path: path.to_owned(),
                value: value.to_data(),
            },
            Input::Samples {
                path,
                times,
                values,
            } => Got::Samples {
                source: source_name,
                path: path.to_owned(),
                times: match times {
                    Times::Period { timestamp, period } => {
                        proto::SampleTimes::Period { timestamp, period }
                    }
                    Times::Each(list) => proto::SampleTimes::Each(list.to_vec()),
                },
                values: values.iter().map(|value| value.to_data()).collect(),
            },
            Input::Latest {
                timestamp,
                path,
                value,
            } => Got::Latest {
                source: source_name,
                timestamp,
                path: path.to_owned(),
                value: value.to_data(),
            },
            Input::Log {
                timestamp,
                path,
                level,
                message,
            } => Got::Log {
                source: source_name,
                timestamp,
                path: path.to_owned(),
                level,
                message: message.to_owned(),
            },
            Input::Metadata(metadata) => Got::Metadata {
                source: source_name,
                metadata: metadata.clone(),
            },
            Input::Actions(actions) => Got::Actions {
                source: source_name,
                actions: actions.to_vec(),
            },
            Input::Clock(clock) => Got::Clock {
                source: source_name,
                clock,
            },
        });
        self.memory.ingest(source, input);
    }
    fn clear(&mut self) {
        self.memory.clear();
    }
    fn reset(&mut self) {
        self.memory.reset();
    }
    fn take_notices(&mut self) -> Vec<(Level, String)> {
        self.memory.take_notices()
    }
}

impl Store for Recorded {
    fn paths_version(&self) -> crate::store::Version {
        self.memory.paths_version()
    }
    fn samples_version(&self) -> crate::store::Version {
        self.memory.samples_version()
    }
    fn pending(&self) -> bool {
        self.memory.pending()
    }
    fn origin(&self) -> Option<u64> {
        self.memory.origin()
    }
    fn time_base(&self) -> Option<TimeBase> {
        self.memory.time_base()
    }
    fn time_span(&self) -> Option<(u64, u64)> {
        self.memory.time_span()
    }
    fn activity(&self, start: u64, step: u64, bins: usize) -> Vec<crate::store::Activity> {
        self.memory.activity(start, step, bins)
    }
    fn traces(&self) -> Box<dyn Iterator<Item = Trace<'_>> + '_> {
        Store::traces(&self.memory)
    }
    fn kinds(&self) -> u64 {
        self.memory.kinds()
    }
    fn trace(&self, path: &str, window: Window) -> Loaded<Option<Trace<'_>>> {
        self.memory.trace(path, window)
    }
    fn series(&self, path: &str, window: Window) -> Loaded<Option<Series<'_>>> {
        self.memory.series(path, window)
    }
    fn field_paths<'a>(&'a self, path: &str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        self.memory.field_paths(path)
    }
    fn metadata(&self, path: &str) -> Option<&proto::TraceMetadata> {
        self.memory.metadata(path)
    }
    fn unit(&self, path: &str) -> Option<&str> {
        self.memory.unit(path)
    }
    fn description(&self, path: &str) -> Option<&str> {
        self.memory.description(path)
    }
    fn labels(&self, path: &str) -> Option<&Labels> {
        self.memory.labels(path)
    }
    fn expected(&self, path: &str) -> Option<(f64, f64)> {
        self.memory.expected(path)
    }
    fn logs(&self, window: Window) -> Loaded<&Logs> {
        self.memory.logs(window)
    }
    fn actions(&self) -> &Actions {
        self.memory.actions()
    }
    fn readback(&self, readback: &str) -> Option<Trace<'_>> {
        self.memory.readback(readback)
    }
    fn readback_samples(&self, readback: Option<&str>) -> usize {
        self.memory.readback_samples(readback)
    }
}

/// A source that hands over typed data of every kind on its first poll, and answers
/// each invocation sent to it on a later one.
struct Typed {
    sent: Rc<RefCell<Vec<proto::Request>>>,
    polls: usize,
    answered: usize,
}

impl Source for Typed {
    fn poll(&mut self, inbox: &mut Inbox<'_>) {
        self.polls += 1;
        if self.polls == 1 {
            inbox.opened();
            inbox.hello(&proto::Hello {
                version: 1,
                name: Some("app".to_owned()),
                clock: Some(proto::Clock::Boot),
                sender: None,
            });
            inbox.metadata(&proto::TraceMetadata {
                path: "float".to_owned(),
                unit: "V".to_owned(),
                ..Default::default()
            });
            inbox.actions(&[reset_action()]);
            inbox.sample(1, "none", None::<f64>);
            inbox.sample(2, "int", -3i64);
            inbox.sample(3, "float", 2.5);
            inbox.sample(4, "bool", true);
            inbox.sample(5, "text", "on");
            inbox.sample(6, "array", [1.0, 2.0]);
            inbox.sample(7, "matrix", [Some([1.0, 2.0]), None, Some([3.0, 4.0])]);
            inbox.sample(
                8,
                "record",
                record(|r| {
                    r.field("t", 21.5);
                    r.field(
                        "pos",
                        record(|p| {
                            p.field("x", 1.0);
                            p.field("y", 2.0);
                        }),
                    );
                }),
            );
            inbox.sample(9, "strings", ["a", "b"]);
            inbox.samples(
                "adc",
                Times::Period {
                    timestamp: 10,
                    period: 5,
                },
                [1u64, 2, 3],
            );
            inbox.samples("flags", Times::Each(&[20, 21]), [true, false]);
            inbox.log(30, "motor", proto::level::WARN, "hot");
            inbox.log(31, "", proto::level::INFO, "up");
        }
        let sent = self.sent.borrow();
        for request in &sent[self.answered..] {
            let proto::Request::Invoke(invocation) = request;
            inbox.action_result(&proto::ActionResult {
                id: invocation.id,
                result: Ok(()),
            });
        }
        self.answered = sent.len();
    }

    fn send(&mut self, request: &proto::Request) {
        self.sent.borrow_mut().push(request.clone());
    }

    fn close(&mut self) {}
}

fn action(
    path: &str,
    readback: Option<&str>,
    params: Vec<proto::Param>,
) -> proto::ActionDescriptor {
    proto::ActionDescriptor {
        path: path.to_owned(),
        description: String::new(),
        params,
        readback: readback.map(str::to_owned),
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }
}

fn reset_action() -> proto::ActionDescriptor {
    action("reset", None, vec![])
}

#[test]
fn a_store_of_the_application_takes_typed_data() {
    use crate::store::actions::{ActionId, Outcome};
    use proto::Data::{Array, Bool, Float, Int, Null, Record, String as Text, Uint};
    let sent = Rc::new(RefCell::new(Vec::new()));
    let mut session = Session::new(Recorded::default());
    session.recording = true;
    session.add_source(
        "app",
        Typed {
            sent: sent.clone(),
            polls: 0,
            answered: 0,
        },
    );
    session.update(0.0);

    let app = || "app".to_owned();
    let sample = |timestamp: u64, path: &str, value: proto::Data| Got::Sample {
        source: app(),
        timestamp,
        path: path.to_owned(),
        value,
    };
    let expected = vec![
        Got::Clock {
            source: app(),
            clock: Some(TimeBase::SinceBoot),
        },
        Got::Metadata {
            source: app(),
            metadata: proto::TraceMetadata {
                path: "float".to_owned(),
                unit: "V".to_owned(),
                ..Default::default()
            },
        },
        Got::Actions {
            source: app(),
            actions: vec![reset_action()],
        },
        sample(1, "none", Null),
        sample(2, "int", Int(-3)),
        sample(3, "float", Float(2.5)),
        sample(4, "bool", Bool(true)),
        sample(5, "text", Text("on".to_owned())),
        sample(6, "array", Array(vec![Float(1.0), Float(2.0)])),
        sample(
            7,
            "matrix",
            Array(vec![
                Array(vec![Float(1.0), Float(2.0)]),
                Null,
                Array(vec![Float(3.0), Float(4.0)]),
            ]),
        ),
        sample(
            8,
            "record",
            Record(vec![
                ("t".to_owned(), Float(21.5)),
                (
                    "pos".to_owned(),
                    Record(vec![
                        ("x".to_owned(), Float(1.0)),
                        ("y".to_owned(), Float(2.0)),
                    ]),
                ),
            ]),
        ),
        sample(
            9,
            "strings",
            Array(vec![Text("a".to_owned()), Text("b".to_owned())]),
        ),
        Got::Samples {
            source: app(),
            path: "adc".to_owned(),
            times: proto::SampleTimes::Period {
                timestamp: 10,
                period: 5,
            },
            values: vec![Uint(1), Uint(2), Uint(3)],
        },
        Got::Samples {
            source: app(),
            path: "flags".to_owned(),
            times: proto::SampleTimes::Each(vec![20, 21]),
            values: vec![Bool(true), Bool(false)],
        },
        Got::Log {
            source: app(),
            timestamp: 30,
            path: "motor".to_owned(),
            level: 3,
            message: "hot".to_owned(),
        },
        Got::Log {
            source: app(),
            timestamp: 31,
            path: String::new(),
            level: 2,
            message: "up".to_owned(),
        },
    ];
    assert_eq!(session.store.got, expected);
    assert_eq!(session.recorder.messages(), 16);
    assert_eq!(session.store.unit("float"), Some("V"));
    let float = session
        .store
        .series("float", Window::of(None))
        .data
        .expect("the float trace");
    assert_eq!(float.timestamps, [3]);
    assert_eq!(float.range, Some((2.5, 2.5)));

    let reset = ActionId::new("app", "reset");
    session.invoker.invoke(&reset, None, vec![], 0.0);
    session.send_invocations();
    session.update(0.1);
    assert_eq!(
        session.invoker.outcome(&reset, 0.1),
        Some(&Outcome::Accepted(0.1))
    );
    assert_eq!(session.recorder.messages(), 17);
    assert_eq!(session.store.got, expected, "the answer reaches no store");
}

/// Every kind of message, as typed data and as the bytes of the protocol, with the source
/// it comes from. Part A announces; part B comes after the invocations.
type Agreed = Vec<(&'static str, proto::Message, Vec<u8>)>;

fn agreement_messages() -> (Agreed, usize) {
    use proto::Data::{Array, Bool, Float, Int, Null, Record, String as Text, Uint};
    #[derive(serde::Serialize)]
    enum Old {
        Value(proto::Sample),
        ValueMetadata { path: String, unit: String },
    }
    fn same(
        source: &'static str,
        typed: proto::Message,
    ) -> (&'static str, proto::Message, Vec<u8>) {
        let bytes = serde_cbor_2::to_vec(&typed).expect("bytes");
        (source, typed, bytes)
    }
    fn hello(
        clock: Option<proto::Clock>,
        name: Option<&str>,
        sender: Option<&str>,
    ) -> proto::Message {
        proto::Message::Hello(proto::Hello {
            version: 1,
            name: name.map(str::to_owned),
            clock,
            sender: sender.map(str::to_owned),
        })
    }
    fn at(timestamp: u64, path: &str, value: proto::Data) -> proto::Message {
        proto::Message::Sample(proto::Sample {
            timestamp,
            path: path.to_owned(),
            value,
        })
    }
    fn log(timestamp: u64, path: &str, level: u8, message: &str) -> proto::Message {
        proto::Message::Log(proto::LogRecord {
            timestamp,
            path: path.to_owned(),
            level,
            message: message.to_owned(),
        })
    }
    let mut list = vec![
        same(
            "a",
            hello(Some(proto::Clock::Boot), Some("rig"), Some("sonde 1.2")),
        ),
        same(
            "a",
            proto::Message::TraceMetadata(proto::TraceMetadata {
                path: "motor/rpm".to_owned(),
                unit: "rpm".to_owned(),
                description: "Speed".to_owned(),
                labels: [(0, "off".to_owned()), (1, "on".to_owned())].into(),
                range: Some(proto::ExpectedRange {
                    min: 0.0,
                    max: 100.0,
                }),
            }),
        ),
        same(
            "a",
            proto::Message::TraceMetadata(proto::TraceMetadata {
                path: "motor/state".to_owned(),
                ..Default::default()
            }),
        ),
        same(
            "a",
            proto::Message::Actions(vec![
                action(
                    "reset",
                    Some("motor/rpm"),
                    vec![proto::Param {
                        name: "rpm".to_owned(),
                        kind: proto::ParamKind::Float {
                            min: Some(0.0),
                            max: None,
                            step: None,
                        },
                        unit: "rpm".to_owned(),
                        default: Some(Float(1.0)),
                    }],
                ),
                action("stop", None, vec![]),
            ]),
        ),
    ];
    let part_a = list.len();
    let samples = [
        (20, "null", Null),
        (21, "uint", Uint(7)),
        (22, "int", Int(-7)),
        (23, "float", Float(0.1)),
        (24, "bool", Bool(true)),
        (25, "text", Text("on".to_owned())),
        (26, "array", Array(vec![Float(1.0), Null, Float(3.0)])),
        (
            27,
            "matrix",
            Array(vec![
                Array(vec![Float(1.0), Float(2.0)]),
                Null,
                Array(vec![Float(3.0), Float(4.0)]),
            ]),
        ),
        (
            28,
            "record",
            Record(vec![
                ("t".to_owned(), Float(21.5)),
                (
                    "acc".to_owned(),
                    Record(vec![
                        ("x".to_owned(), Float(0.5)),
                        ("y".to_owned(), Int(-1)),
                    ]),
                ),
                ("mode".to_owned(), Text("run".to_owned())),
            ]),
        ),
        (
            29,
            "strings",
            Array(vec![Text("a".to_owned()), Text("b".to_owned())]),
        ),
        (
            30,
            "records",
            Array(vec![
                Record(vec![("a".to_owned(), Uint(1))]),
                Record(vec![("a".to_owned(), Uint(2))]),
            ]),
        ),
    ];
    list.push(same(
        "a",
        proto::Message::ActionResult(proto::ActionResult {
            id: 0,
            result: Ok(()),
        }),
    ));
    list.push(same(
        "a",
        proto::Message::ActionResult(proto::ActionResult {
            id: 1,
            result: Err("jammed".to_owned()),
        }),
    ));
    list.push(same("a", log(10, "motor", proto::level::WARN, "hot")));
    list.push(same("a", log(11, "", proto::level::INFO, "up")));
    for (timestamp, path, value) in samples {
        list.push(same("a", at(timestamp, path, value)));
    }
    list.push(same(
        "a",
        proto::Message::Samples(proto::Samples {
            path: "adc".to_owned(),
            values: vec![Uint(1), Uint(2), Uint(3)],
            times: proto::SampleTimes::Period {
                timestamp: 40,
                period: 5,
            },
        }),
    ));
    list.push(same(
        "a",
        proto::Message::Samples(proto::Samples {
            path: "flags".to_owned(),
            values: vec![Bool(true), Bool(false)],
            times: proto::SampleTimes::Each(vec![50, 52]),
        }),
    ));
    let old = proto::Sample {
        timestamp: 60,
        path: "old".to_owned(),
        value: Float(1.5),
    };
    let bytes = serde_cbor_2::to_vec(&Old::Value(old.clone())).expect("bytes");
    list.push(("a", proto::Message::Sample(old), bytes));
    let bytes = serde_cbor_2::to_vec(&Old::ValueMetadata {
        path: "old".to_owned(),
        unit: "V".to_owned(),
    })
    .expect("bytes");
    list.push((
        "a",
        proto::Message::TraceMetadata(proto::TraceMetadata {
            path: "old".to_owned(),
            unit: "V".to_owned(),
            ..Default::default()
        }),
        bytes,
    ));
    list.push(same("b", hello(Some(proto::Clock::Epoch), None, None)));
    list.push(same("c", hello(None, None, None)));
    (list, part_a)
}

/// What a session holds after the messages: what its store got, the outcomes of the two
/// invocations, the notices, and the recording.
use crate::store::actions::Outcome;

struct Held {
    got: Vec<Got>,
    outcomes: (Option<Outcome>, Option<Outcome>),
    notices: Vec<(Level, String)>,
    file: Vec<u8>,
    cleared: Vec<u8>,
    session: Session<Recorded>,
}

fn hold(recording: bool, typed: bool) -> Held {
    use crate::store::actions::ActionId;
    let (list, part_a) = agreement_messages();
    let mut session = Session::new(Recorded::default());
    session.recording = recording;
    let feed =
        |session: &mut Session<Recorded>, messages: &[(&str, proto::Message, Vec<u8>)], now| {
            for (source, message, bytes) in messages {
                let mut inbox = session.inbox(source, now);
                if typed {
                    inbox.message(message);
                } else {
                    inbox.frame(bytes);
                }
                inbox.finish();
            }
        };
    feed(&mut session, &list[..part_a], 0.0);
    session
        .invoker
        .invoke(&ActionId::new("a", "reset"), None, vec![], 0.0);
    session
        .invoker
        .invoke(&ActionId::new("a", "stop"), None, vec![], 0.0);
    feed(&mut session, &list[part_a..], 1.0);
    let outcomes = (
        session
            .invoker
            .outcome(&ActionId::new("a", "reset"), 1.0)
            .cloned(),
        session
            .invoker
            .outcome(&ActionId::new("a", "stop"), 1.0)
            .cloned(),
    );
    let notices = session.take_notices();
    let file = session.recorder.to_file();
    session.clear_data();
    let cleared = session.recorder.to_file();
    let got = std::mem::take(&mut session.store.got);
    Held {
        got,
        outcomes,
        notices,
        file,
        cleared,
        session,
    }
}

#[test]
fn the_parser_and_the_typed_way_agree() {
    use proto::Data::{Bool, Uint};
    for recording in [true, false] {
        let typed = hold(recording, true);
        let bytes = hold(recording, false);
        assert_eq!(typed.got, bytes.got, "the store, recording {recording}");
        assert_eq!(
            typed.outcomes, bytes.outcomes,
            "the invoker, recording {recording}"
        );
        assert_eq!(
            typed.outcomes,
            (
                Some(Outcome::Accepted(1.0)),
                Some(Outcome::Rejected("jammed".to_owned()))
            )
        );
        assert_eq!(
            typed.notices, bytes.notices,
            "the notices, recording {recording}"
        );
        assert_eq!(
            typed.file, bytes.file,
            "the recording, recording {recording}"
        );
        assert_eq!(
            typed.cleared, bytes.cleared,
            "the recording after clearing, recording {recording}"
        );
        assert_eq!(
            typed.session.recorder.messages(),
            bytes.session.recorder.messages()
        );

        let kept = |got: &Vec<Got>| {
            got.iter()
                .filter(|got| {
                    matches!(
                        got,
                        Got::Sample { .. } | Got::Samples { .. } | Got::Log { .. }
                    )
                })
                .count()
        };
        let latest = typed
            .got
            .iter()
            .filter(|got| matches!(got, Got::Latest { .. }))
            .count();
        if recording {
            assert_eq!(latest, 0);
            assert_eq!(kept(&typed.got), 11 + 1 + 2 + 2);
            assert!(!typed.file.is_empty());
        } else {
            assert_eq!(kept(&typed.got), 0, "only the latest values are kept");
            // each sample, with the old name, and the last row of each block
            assert_eq!(latest, 11 + 1 + 2);
            let last = |timestamp: u64, path: &str, value| Got::Latest {
                source: "a".to_owned(),
                timestamp,
                path: path.to_owned(),
                value,
            };
            assert!(typed.got.contains(&last(50, "adc", Uint(3))));
            assert!(typed.got.contains(&last(52, "flags", Bool(false))));
        }
    }
}

/// The bytes of `message` as a source of the protocol sends them.
fn serde(message: &proto::Message) -> Vec<u8> {
    serde_cbor_2::to_vec(message).expect("bytes")
}

/// A block with both a period and timestamps, written by hand: the parser refuses it.
fn malformed_block() -> Vec<u8> {
    let mut both = vec![0xa1, 0x67];
    both.extend(b"Samples");
    both.extend([0xa5, 0x64]);
    both.extend(b"path");
    both.push(0x63);
    both.extend(b"bad");
    both.push(0x66);
    both.extend(b"values");
    both.extend([0x81, 0x01, 0x69]);
    both.extend(b"timestamp");
    both.extend([0x01, 0x66]);
    both.extend(b"period");
    both.extend([0x01, 0x6a]);
    both.extend(b"timestamps");
    both.extend([0x81, 0x01]);
    both
}

fn log_message(timestamp: u64, path: &str, level: u8, message: &str) -> proto::Message {
    proto::Message::Log(proto::LogRecord {
        timestamp,
        path: path.to_owned(),
        level,
        message: message.to_owned(),
    })
}

fn sample_message(timestamp: u64, path: &str, value: proto::Data) -> proto::Message {
    proto::Message::Sample(proto::Sample {
        timestamp,
        path: path.to_owned(),
        value,
    })
}

/// The last value of a readback as the widgets show it.
fn readback_value(store: &impl Store, path: &str) -> Option<String> {
    let trace = store.readback(path)?;
    let last = trace.timestamps.len().checked_sub(1)?;
    Some(trace.samples.format(
        last,
        4,
        None,
        crate::store::numbers::DisplayFormat::Default,
        None,
    ))
}

/// A source that hands over the same data as bytes or as typed calls.
struct Feeder {
    bytes: bool,
    polls: usize,
}

impl Source for Feeder {
    fn poll(&mut self, inbox: &mut Inbox<'_>) {
        use proto::Data::{String as Text, Uint};
        self.polls += 1;
        if self.polls == 1 {
            inbox.opened();
            let actions = [
                action("set_rpm", Some("rpm"), vec![]),
                action("set_adc", Some("adc"), vec![]),
                action("set_level", Some("level"), vec![]),
            ];
            if self.bytes {
                let block = proto::Message::Samples(proto::Samples {
                    path: "adc".to_owned(),
                    values: vec![Uint(1), Uint(2), Uint(3)],
                    times: proto::SampleTimes::Each(vec![4, 5, 6]),
                });
                for message in [
                    sample_message(1, "level", Text("INFO".to_owned())),
                    proto::Message::Actions(actions.to_vec()),
                    sample_message(2, "rpm", Uint(5)),
                    sample_message(3, "rpm", Uint(7)),
                    block,
                    log_message(7, "motor", proto::level::WARN, "hot"),
                ] {
                    inbox.frame(&serde(&message));
                }
            } else {
                inbox.sample(1, "level", "INFO");
                inbox.actions(&actions);
                inbox.sample(2, "rpm", 5u64);
                inbox.sample(3, "rpm", 7u64);
                inbox.samples("adc", Times::Each(&[4, 5, 6]), [1u64, 2, 3]);
                inbox.log(7, "motor", proto::level::WARN, "hot");
            }
        } else if self.bytes {
            inbox.frame(&serde(&sample_message(10, "rpm", Uint(9))));
        } else {
            inbox.sample(10, "rpm", 9u64);
        }
    }

    fn send(&mut self, _request: &proto::Request) {}

    fn close(&mut self) {}
}

#[test]
fn samples_are_not_kept_while_not_recording_but_the_latest_value_is() {
    for bytes in [true, false] {
        let mut session = Session::default();
        session.add_source("feeder", Feeder { bytes, polls: 0 });
        session.update(0.0);
        let store = &session.store;
        assert!(!store.has_samples(), "bytes {bytes}");
        assert_eq!(store.logs.paths().next(), None, "bytes {bytes}");
        assert_eq!(readback_value(store, "rpm").as_deref(), Some("7"));
        assert_eq!(store.readback_samples(Some("rpm")), 2, "bytes {bytes}");
        assert_eq!(readback_value(store, "adc").as_deref(), Some("3"));
        assert_eq!(store.readback_samples(Some("adc")), 1, "bytes {bytes}");
        // arrived before its path became a readback
        assert_eq!(readback_value(store, "level").as_deref(), Some("INFO"));
        assert_eq!(session.recorder.messages(), 0, "bytes {bytes}");

        // recording starts from what was received, as before
        assert!(session.set_recording(true, true));
        assert_eq!(session.recorder.messages(), 1, "bytes {bytes}");
        session.update(0.1);
        let rpm = session.store.traces.get("rpm").expect("rpm");
        assert_eq!(rpm.timestamps.len(), 1, "bytes {bytes}");
        assert_eq!(rpm.timestamps[0], 10, "bytes {bytes}");
        assert_eq!(session.recorder.messages(), 2, "bytes {bytes}");
    }
}

#[test]
fn what_arrives_live_is_recorded_without_loss() {
    use proto::Data::{Array, Float, String as Text, Uint};
    #[derive(serde::Serialize)]
    enum Old {
        Value(proto::Sample),
        ValueMetadata { path: String, unit: String },
    }
    #[derive(serde::Serialize)]
    struct Header {
        format: String,
        version: u32,
    }
    let metadata = |unit: &str| proto::TraceMetadata {
        path: "app/temp".to_owned(),
        unit: unit.to_owned(),
        ..Default::default()
    };
    let hello = proto::Hello {
        version: 1,
        name: Some("rig".to_owned()),
        clock: Some(proto::Clock::Boot),
        sender: None,
    };
    let adc = proto::Message::Samples(proto::Samples {
        path: "adc".to_owned(),
        values: vec![Uint(1), Uint(2), Uint(3)],
        times: proto::SampleTimes::Period {
            timestamp: 40,
            period: 5,
        },
    });
    let flags = proto::Message::Samples(proto::Samples {
        path: "flags".to_owned(),
        values: vec![proto::Data::Bool(true), proto::Data::Bool(false)],
        times: proto::SampleTimes::Each(vec![50, 52]),
    });
    let strings = sample_message(
        6,
        "strings",
        Array(vec![Text("a".into()), Text("b".into())]),
    );
    let records = proto::Data::Array(vec![
        proto::Data::Record(vec![("a".to_owned(), Uint(1))]),
        proto::Data::Record(vec![("a".to_owned(), Uint(2))]),
    ]);
    let old_sample = proto::Sample {
        timestamp: 9,
        path: "old".to_owned(),
        value: Float(1.5),
    };
    let old_value = serde_cbor_2::to_vec(&Old::Value(old_sample.clone())).expect("bytes");
    let old_metadata = serde_cbor_2::to_vec(&Old::ValueMetadata {
        path: "old".to_owned(),
        unit: "V".to_owned(),
    })
    .expect("bytes");
    let old_as_metadata = proto::TraceMetadata {
        path: "old".to_owned(),
        unit: "V".to_owned(),
        ..Default::default()
    };
    let future = vec![0xa1, 0x66, b'F', b'u', b't', b'u', b'r', b'e', 0x01];
    let malformed = malformed_block();
    let result = proto::ActionResult {
        id: 9,
        result: Err("no".to_owned()),
    };
    let reset = proto::Message::Actions(vec![reset_action()]);

    let mut session = Session::recording();
    let wire = |session: &mut Session<MemoryStore>, bytes: &[u8]| {
        session.inbox("wire", 0.0).frame(bytes);
    };
    wire(&mut session, &serde(&proto::Message::Hello(hello.clone())));
    session.inbox("app", 0.0).metadata(&metadata("°C"));
    wire(&mut session, &serde(&reset));
    session.inbox("app", 0.0).action_result(&result);
    let mut frame = serde(&log_message(5, "", proto::level::INFO, "up"));
    frame.extend(serde(&strings));
    wire(&mut session, &frame);
    {
        let mut inbox = session.inbox("app", 0.0);
        inbox.sample(
            7,
            "records",
            [1u64, 2].map(|n| record(move |r| r.field("a", n))),
        );
        inbox.log(8, "app", proto::level::WARN, "hot");
    }
    wire(&mut session, &serde(&adc));
    wire(&mut session, &serde(&flags));
    wire(&mut session, &old_value);
    wire(&mut session, &old_metadata);
    let mut frame = serde(&log_message(10, "", proto::level::INFO, "x"));
    frame.extend(&malformed);
    frame.extend(serde(&sample_message(11, "rpm", Uint(5))));
    wire(&mut session, &frame);
    wire(&mut session, &future);

    let mut expected: Vec<(&str, Vec<u8>)> = vec![
        ("wire", serde(&proto::Message::Hello(hello))),
        ("app", serde(&proto::Message::TraceMetadata(metadata("°C")))),
        ("wire", serde(&reset)),
        ("app", serde(&proto::Message::ActionResult(result))),
        ("wire", serde(&log_message(5, "", proto::level::INFO, "up"))),
        ("wire", serde(&strings)),
        ("app", serde(&sample_message(7, "records", records))),
        (
            "app",
            serde(&log_message(8, "app", proto::level::WARN, "hot")),
        ),
        ("wire", serde(&adc)),
        ("wire", serde(&flags)),
        ("wire", serde(&proto::Message::Sample(old_sample))),
        (
            "wire",
            serde(&proto::Message::TraceMetadata(old_as_metadata.clone())),
        ),
        ("wire", serde(&log_message(10, "", proto::level::INFO, "x"))),
        ("wire", malformed.clone()),
        ("wire", serde(&sample_message(11, "rpm", Uint(5)))),
        ("wire", future.clone()),
    ];
    let read = |file: &[u8]| {
        let mut items = vec![];
        let loaded = read_file(file, |source, item| {
            items.push((source.to_owned(), item.to_vec()));
        });
        assert_eq!(loaded.expect("loads").problem, None);
        items
    };
    let want = |expected: &[(&str, Vec<u8>)]| -> Vec<(String, Vec<u8>)> {
        expected
            .iter()
            .map(|(source, item)| ((*source).to_owned(), item.clone()))
            .collect()
    };
    let file = session.recorder.to_file();
    assert_eq!(read(&file), want(&expected));
    // the file starts with the header
    let mut plain = vec![];
    std::io::Read::read_to_end(
        &mut flate2::read::MultiGzDecoder::new(&file[..]),
        &mut plain,
    )
    .expect("gzip");
    let header = serde_cbor_2::to_vec(&Header {
        format: "blip recording".to_owned(),
        version: 3,
    })
    .expect("header");
    assert!(plain.starts_with(&header));
    assert_eq!(
        session.take_notices(),
        [(
            Level::Error,
            "Dropped a malformed block of samples from wire".to_owned()
        )]
    );
    assert!(session.store.traces.get("bad").is_none());
    expected.clear();

    // a recording that starts later begins with the latest descriptions
    session.recording = false;
    let rig2 = proto::Hello {
        name: Some("rig2".to_owned()),
        ..Default::default()
    };
    wire(&mut session, &serde(&proto::Message::Hello(rig2.clone())));
    session.inbox("app", 1.0).metadata(&metadata("K"));
    assert!(session.set_recording(true, true));
    assert_eq!(
        read(&session.recorder.to_file()),
        want(&[
            ("wire", serde(&proto::Message::Hello(rig2))),
            ("app", serde(&proto::Message::TraceMetadata(metadata("K")))),
            ("wire", serde(&reset)),
            (
                "wire",
                serde(&proto::Message::TraceMetadata(old_as_metadata))
            ),
        ])
    );
}

#[test]
fn a_malformed_block_is_dropped_alone() {
    let both = malformed_block();
    let mut sequence = serde(&log_message(10, "", 2, "hi"));
    sequence.extend(&both);
    sequence.extend(serde(&sample_message(20, "rpm", proto::Data::Uint(5))));
    let mut session = Session::recording();
    session.inbox("a", 0.0).frame(&sequence);
    assert_eq!(session.store.logs.entries().count(), 1);
    assert!(session.store.traces.get("rpm").is_some());
    assert!(session.store.traces.get("bad").is_none());
    assert_eq!(
        session.take_notices(),
        [(
            Level::Error,
            "Dropped a malformed block of samples from a".to_owned()
        )]
    );
    assert_eq!(session.take_notices(), []);
    session.inbox("", 0.0).frame(&both);
    assert_eq!(
        session.take_notices(),
        [(
            Level::Error,
            "Dropped a malformed block of samples".to_owned()
        )]
    );
}
