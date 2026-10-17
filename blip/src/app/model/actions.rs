//! The actions the targets announced, and the invocations blip sends to carry them out.

use super::Model;
use proto::{ActionDescriptor, ActionResult, Data, Invocation, Request};
use std::collections::{HashMap, HashSet};

/// Seconds after which an invocation without an answer counts as unanswered.
pub const ANSWER_TIMEOUT: f64 = 5.0;
/// Seconds an accepted invocation is shown as accepted.
pub const SHOW_ACCEPTED: f64 = 1.5;
/// The least seconds between the invocations sent while a slider is dragged.
pub const DRAG_INTERVAL: f64 = 0.1;

/// An action announced by the outpost of a connection.
pub struct Announced {
    /// The name of the connection.
    pub source: String,
    /// The action, with its path and readback as the outpost sent them.
    pub descriptor: ActionDescriptor,
}

/// The actions as last announced by the outpost of each connection. Several outposts
/// may announce an action at the same path, each is kept.
#[derive(Default)]
pub struct Actions {
    /// The actions of each source together, in the order they were announced first.
    list: Vec<Announced>,
}

impl Actions {
    /// Replaces the actions of `source`, whose outpost announces all of them each time.
    /// They keep their place among the actions of other sources.
    pub fn set(&mut self, source: &str, actions: Vec<ActionDescriptor>) {
        let at = self.list.iter().position(|action| action.source == source);
        self.list.retain(|action| action.source != source);
        let at = at.unwrap_or(self.list.len());
        let announced = actions.into_iter().map(|descriptor| Announced {
            source: source.to_owned(),
            descriptor,
        });
        self.list.splice(at..at, announced);
    }

    /// The paths of the actions, each once, in the order they were announced first.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        let mut seen = HashSet::new();
        self.list
            .iter()
            .map(|action| action.descriptor.path.as_str())
            .filter(move |path| seen.insert(*path))
    }

    /// The names of the connections whose outposts announced actions, each once, in the
    /// order they announced first.
    pub fn sources(&self) -> impl Iterator<Item = &str> {
        let mut seen = HashSet::new();
        self.list
            .iter()
            .map(|action| action.source.as_str())
            .filter(move |source| seen.insert(*source))
    }

    /// The announcements of the action at `path`, one per outpost announcing it, in
    /// the order they were announced first.
    pub fn announcers(&self, path: &str) -> impl Iterator<Item = &Announced> {
        self.list
            .iter()
            .filter(move |action| action.descriptor.path == path)
    }

    /// The action at `path` as the outpost of the connection `source` announced it.
    pub fn announced(&self, source: &str, path: &str) -> Option<&ActionDescriptor> {
        self.announcers(path)
            .find(|action| action.source == source)
            .map(|action| &action.descriptor)
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

/// An action of the outpost of a connection, to which its invocations are sent.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ActionId {
    /// The name of the connection.
    pub source: String,
    /// The path of the action.
    pub path: String,
}

impl ActionId {
    pub fn new(source: &str, path: &str) -> Self {
        Self {
            source: source.to_owned(),
            path: path.to_owned(),
        }
    }
}

/// The arguments of an invocation by parameter name.
pub type Args = Vec<(String, Data)>;

/// What became of the latest invocation of an action.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Pending,
    /// Carried out at the time.
    Accepted(f64),
    /// Rejected by the target for the reason.
    Rejected(String),
    /// Not answered within [`ANSWER_TIMEOUT`].
    Unanswered,
}

/// What was sent of an action while its input is dragged.
struct Dragged {
    /// When the latest arguments were sent.
    sent_at: f64,
    sent: Args,
    /// Newer arguments held back to keep to [`DRAG_INTERVAL`].
    held: Option<Args>,
    /// The path of the readback trace of the action.
    readback: Option<String>,
}

/// An invocation without an answer.
struct Pending {
    action: ActionId,
    /// The path of the readback trace of the action, to mark its samples when the
    /// invocation is accepted.
    readback: Option<String>,
    /// When it was sent.
    sent_at: f64,
}

/// Sends the invocations of actions and keeps track of their answers. Times are in
/// seconds, as egui counts them.
#[derive(Default)]
pub struct Invoker {
    /// The names of the open connections, whose actions can be invoked.
    pub connected: HashSet<String>,
    next_id: u32,
    /// The invocations without answer, by id.
    pending: HashMap<u32, Pending>,
    /// The id of the latest invocation of each action and its outcome.
    latest: HashMap<ActionId, (u32, Outcome)>,
    /// The number of samples the readback trace of each action had when the outpost
    /// accepted its latest invocation. Samples up to then may show the state before
    /// the action.
    readback_marks: HashMap<ActionId, usize>,
    /// The actions whose input is dragged.
    dragged: HashMap<ActionId, Dragged>,
    /// The requests to send, each with the name of the connection to send it to.
    outbox: Vec<(String, Request)>,
    /// The values in the inputs of the parameters by action and parameter name, shared
    /// by the views showing an action on the same outpost.
    pub inputs: HashMap<(ActionId, String), Data>,
}

impl Invoker {
    /// Asks the outpost to carry out `action`, whose state shows in the trace at
    /// `readback`, if any.
    pub fn invoke(&mut self, action: &ActionId, readback: Option<&str>, args: Args, now: f64) {
        self.dragged.remove(action);
        self.send(action, readback, args, now);
    }

    /// Invokes `action` while an input is dragged, at most every [`DRAG_INTERVAL`].
    /// Arguments held back are sent by [`Self::update`] once the interval has passed,
    /// unless newer ones replace them.
    pub fn invoke_dragging(
        &mut self,
        action: &ActionId,
        readback: Option<&str>,
        args: Args,
        now: f64,
    ) {
        match self.dragged.get_mut(action) {
            Some(dragged) if now - dragged.sent_at < DRAG_INTERVAL => {
                dragged.held = (args != dragged.sent).then_some(args);
            }
            _ => self.send_dragged(action, readback, args, now),
        }
    }

    /// Sends the final arguments when the input of `action` is released, unless they
    /// were sent already. Nothing is sent for an input that was pressed without
    /// changing it, e.g. clicked to type a number.
    pub fn finish_dragging(
        &mut self,
        action: &ActionId,
        readback: Option<&str>,
        args: Args,
        now: f64,
    ) {
        if let Some(dragged) = self.dragged.remove(action)
            && args != dragged.sent
        {
            self.send(action, readback, args, now);
        }
    }

    fn send_dragged(&mut self, action: &ActionId, readback: Option<&str>, args: Args, now: f64) {
        self.send(action, readback, args.clone(), now);
        let dragged = Dragged {
            sent_at: now,
            sent: args,
            held: None,
            readback: readback.map(str::to_owned),
        };
        self.dragged.insert(action.clone(), dragged);
    }

    fn send(&mut self, action: &ActionId, readback: Option<&str>, args: Args, now: f64) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let invocation = Invocation {
            id,
            path: action.path.clone(),
            args: Data::Record(args),
        };
        self.outbox
            .push((action.source.clone(), Request::Invoke(invocation)));
        let pending = Pending {
            action: action.clone(),
            readback: readback.map(str::to_owned),
            sent_at: now,
        };
        self.pending.insert(id, pending);
        self.latest.insert(action.clone(), (id, Outcome::Pending));
    }

    /// Whether `action` can be invoked, as the connection of its outpost is open.
    pub fn is_connected(&self, action: &ActionId) -> bool {
        self.connected.contains(&action.source)
    }

    /// Takes the answer to an invocation received from the connection `source`.
    /// Answers from another connection than the one the invocation was sent to are
    /// ignored.
    pub fn receive(&mut self, source: &str, result: &ActionResult, now: f64, model: &Model) {
        let from_outpost = self
            .pending
            .get(&result.id)
            .is_some_and(|pending| pending.action.source == source);
        if !from_outpost {
            return;
        }
        let pending = self.pending.remove(&result.id).expect("pending");
        // only the latest invocation of an action shows
        if let Some((id, outcome)) = self.latest.get_mut(&pending.action)
            && *id == result.id
        {
            *outcome = match &result.result {
                Ok(()) => Outcome::Accepted(now),
                Err(reason) => Outcome::Rejected(reason.clone()),
            };
            if result.result.is_ok() {
                let samples = model.readback_samples(pending.readback.as_deref());
                self.readback_marks.insert(pending.action, samples);
            }
        }
    }

    /// Whether an input of `action` shows the readback, given the number of samples of
    /// its readback trace: not while the latest invocation is pending, and once it
    /// was accepted only from a sample that arrived after the answer, as earlier ones
    /// may show the state before.
    pub fn follows_readback(&self, action: &ActionId, readback_samples: usize) -> bool {
        match self.latest.get(action) {
            Some((_, Outcome::Pending)) => false,
            Some((_, Outcome::Accepted(_))) => self
                .readback_marks
                .get(action)
                .is_none_or(|&mark| readback_samples > mark),
            _ => true,
        }
    }

    /// Sends held back arguments that are due and gives up on invocations that were
    /// not answered in time.
    pub fn update(&mut self, now: f64) {
        let due: Vec<(ActionId, Option<String>, Args)> = self
            .dragged
            .iter_mut()
            .filter(|(_, dragged)| now - dragged.sent_at >= DRAG_INTERVAL)
            .filter_map(|(action, dragged)| {
                let held = dragged.held.take()?;
                Some((action.clone(), dragged.readback.clone(), held))
            })
            .collect();
        for (action, readback, args) in due {
            self.send_dragged(&action, readback.as_deref(), args, now);
        }

        let expired: Vec<u32> = self
            .pending
            .iter()
            .filter(|(_, pending)| now - pending.sent_at > ANSWER_TIMEOUT)
            .map(|(&id, _)| id)
            .collect();
        for id in expired {
            let pending = self.pending.remove(&id).expect("listed as pending");
            if let Some((latest, outcome)) = self.latest.get_mut(&pending.action)
                && *latest == id
            {
                *outcome = Outcome::Unanswered;
            }
        }
    }

    /// The outcome of the latest invocation of `action` to show, none once an accepted
    /// one was shown for [`SHOW_ACCEPTED`].
    pub fn outcome(&self, action: &ActionId, now: f64) -> Option<&Outcome> {
        let (_, outcome) = self.latest.get(action)?;
        match outcome {
            Outcome::Accepted(at) if now - at > SHOW_ACCEPTED => None,
            outcome => Some(outcome),
        }
    }

    /// Whether the outcomes shown change in time without new messages, so blip
    /// repaints.
    pub fn is_waiting(&self, now: f64) -> bool {
        !self.pending.is_empty()
            || self.dragged.values().any(|dragged| dragged.held.is_some())
            || self.latest.values().any(
                |(_, outcome)| matches!(outcome, Outcome::Accepted(at) if now - at <= SHOW_ACCEPTED),
            )
    }

    /// The requests to send, each with the name of the connection to send it to.
    pub fn take_outbox(&mut self) -> Vec<(String, Request)> {
        std::mem::take(&mut self.outbox)
    }

    /// Forgets how many samples the readback traces had, when the data is cleared and
    /// the traces start again from none.
    pub fn forget_readback_marks(&mut self) {
        self.readback_marks.clear();
    }

    /// Forgets the invocations, e.g. when a recording replaces the data. The inputs and
    /// the open connections are kept.
    pub fn reset(&mut self) {
        *self = Self {
            connected: std::mem::take(&mut self.connected),
            inputs: std::mem::take(&mut self.inputs),
            ..Self::default()
        };
    }
}

#[cfg(test)]
mod tests {
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
        }
    }

    /// The action at `path` of the connection without a name.
    fn unnamed(path: &str) -> ActionId {
        ActionId::new("", path)
    }

    /// A model with `samples` samples of the trace "on", the readback of "enable".
    fn model(samples: u64) -> Model {
        let mut model = Model::default();
        for timestamp in 0..samples {
            model.traces.append(&proto::Value {
                timestamp,
                path: "on".to_owned(),
                value: Data::Bool(true),
            });
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
        // after clearing the data, the readback starts again from no samples
        invoker.forget_readback_marks();
        assert!(invoker.follows_readback(&enable, 1));
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
    fn unanswered_invocations_time_out() {
        let mut invoker = Invoker::default();
        let speed = unnamed("speed");
        invoker.invoke(&speed, None, args(1), 0.0);
        invoker.update(ANSWER_TIMEOUT);
        assert_eq!(invoker.outcome(&speed, 0.0), Some(&Outcome::Pending));
        invoker.update(ANSWER_TIMEOUT + 0.1);
        assert_eq!(invoker.outcome(&speed, 0.0), Some(&Outcome::Unanswered));
        assert!(!invoker.is_waiting(ANSWER_TIMEOUT + 0.1));
        // a late answer changes nothing
        invoker.receive("", &answer(0, Ok(())), 6.0, &model(0));
        assert_eq!(invoker.outcome(&speed, 6.0), Some(&Outcome::Unanswered));
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
        assert!(invoker.is_waiting(0.08));
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
}
