//! The actions the target announced, and the invocations blip sends to carry them out.

use proto::{ActionDescriptor, ActionResult, Data, Invocation, Request};
use std::collections::HashMap;

/// Seconds after which an invocation without an answer counts as unanswered.
pub const ANSWER_TIMEOUT: f64 = 5.0;
/// Seconds an accepted invocation is shown as accepted.
pub const SHOW_ACCEPTED: f64 = 1.5;
/// The least seconds between the invocations sent while a slider is dragged.
pub const DRAG_INTERVAL: f64 = 0.1;

/// The actions as last announced by the target.
#[derive(Default)]
pub struct Actions {
    list: Vec<ActionDescriptor>,
}

impl Actions {
    /// Replaces the actions, the target announces all of them each time.
    pub fn set(&mut self, list: Vec<ActionDescriptor>) {
        self.list = list;
    }

    pub fn iter(&self) -> impl Iterator<Item = &ActionDescriptor> {
        self.list.iter()
    }

    pub fn get(&self, path: &str) -> Option<&ActionDescriptor> {
        self.list.iter().find(|action| action.path == path)
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
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
}

/// Sends the invocations of actions and keeps track of their answers. Times are in
/// seconds, as egui counts them.
#[derive(Default)]
pub struct Invoker {
    /// Whether blip is connected to a bridge, so actions can be invoked.
    pub connected: bool,
    next_id: u32,
    /// The paths and send times of the invocations without answer, by id.
    pending: HashMap<u32, (String, f64)>,
    /// The id of the latest invocation of each action and its outcome, by path.
    latest: HashMap<String, (u32, Outcome)>,
    /// The actions whose input is dragged, by path.
    dragged: HashMap<String, Dragged>,
    /// The requests to send.
    outbox: Vec<Request>,
    /// The values in the inputs of the parameters by action path and parameter name,
    /// shared by the views showing an action.
    pub inputs: HashMap<(String, String), Data>,
}

impl Invoker {
    /// Asks the target to carry out the action at `path`.
    pub fn invoke(&mut self, path: &str, args: Args, now: f64) {
        self.dragged.remove(path);
        self.send(path, args, now);
    }

    /// Invokes the action at `path` while an input is dragged, at most every
    /// [`DRAG_INTERVAL`]. Arguments held back are sent by [`Self::update`] once the
    /// interval has passed, unless newer ones replace them.
    pub fn invoke_dragging(&mut self, path: &str, args: Args, now: f64) {
        match self.dragged.get_mut(path) {
            Some(dragged) if now - dragged.sent_at < DRAG_INTERVAL => {
                dragged.held = (args != dragged.sent).then_some(args);
            }
            _ => self.send_dragged(path, args, now),
        }
    }

    /// Sends the final arguments when the input of the action at `path` is released,
    /// unless they were sent already. Nothing is sent for an input that was pressed
    /// without changing it, e.g. clicked to type a number.
    pub fn finish_dragging(&mut self, path: &str, args: Args, now: f64) {
        if let Some(dragged) = self.dragged.remove(path)
            && args != dragged.sent
        {
            self.send(path, args, now);
        }
    }

    fn send_dragged(&mut self, path: &str, args: Args, now: f64) {
        self.send(path, args.clone(), now);
        let dragged = Dragged {
            sent_at: now,
            sent: args,
            held: None,
        };
        self.dragged.insert(path.to_owned(), dragged);
    }

    fn send(&mut self, path: &str, args: Args, now: f64) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.outbox.push(Request::Invoke(Invocation {
            id,
            path: path.to_owned(),
            args: Data::Record(args),
        }));
        self.pending.insert(id, (path.to_owned(), now));
        self.latest.insert(path.to_owned(), (id, Outcome::Pending));
    }

    /// Takes the answer of the target to an invocation.
    pub fn receive(&mut self, result: &ActionResult, now: f64) {
        let Some((path, _)) = self.pending.remove(&result.id) else {
            return;
        };
        // only the latest invocation of an action shows
        if let Some((id, outcome)) = self.latest.get_mut(&path)
            && *id == result.id
        {
            *outcome = match &result.result {
                Ok(()) => Outcome::Accepted(now),
                Err(reason) => Outcome::Rejected(reason.clone()),
            };
        }
    }

    /// Sends held back arguments that are due and gives up on invocations that were
    /// not answered in time.
    pub fn update(&mut self, now: f64) {
        let due: Vec<(String, Args)> = self
            .dragged
            .iter_mut()
            .filter(|(_, dragged)| now - dragged.sent_at >= DRAG_INTERVAL)
            .filter_map(|(path, dragged)| Some((path.clone(), dragged.held.take()?)))
            .collect();
        for (path, args) in due {
            self.send_dragged(&path, args, now);
        }

        let expired: Vec<u32> = self
            .pending
            .iter()
            .filter(|(_, (_, sent))| now - sent > ANSWER_TIMEOUT)
            .map(|(&id, _)| id)
            .collect();
        for id in expired {
            let (path, _) = self.pending.remove(&id).expect("listed as pending");
            if let Some((latest, outcome)) = self.latest.get_mut(&path)
                && *latest == id
            {
                *outcome = Outcome::Unanswered;
            }
        }
    }

    /// The outcome of the latest invocation of the action at `path` to show, none once
    /// an accepted one was shown for [`SHOW_ACCEPTED`].
    pub fn outcome(&self, path: &str, now: f64) -> Option<&Outcome> {
        let (_, outcome) = self.latest.get(path)?;
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

    /// The requests to send.
    pub fn take_outbox(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.outbox)
    }

    /// Forgets the invocations, e.g. when the connection ends. The inputs are kept.
    pub fn reset(&mut self, connected: bool) {
        *self = Self {
            connected,
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

    fn sent(invoker: &mut Invoker) -> Vec<(u32, String, Data)> {
        invoker
            .take_outbox()
            .into_iter()
            .map(|Request::Invoke(invocation)| (invocation.id, invocation.path, invocation.args))
            .collect()
    }

    fn answer(id: u32, result: Result<(), &str>) -> ActionResult {
        ActionResult {
            id,
            result: result.map_err(str::to_owned),
        }
    }

    #[test]
    fn invocations_are_sent_with_new_ids_and_answered() {
        let mut invoker = Invoker::default();
        invoker.invoke("speed", args(1), 0.0);
        invoker.invoke("reboot", vec![], 0.0);
        assert_eq!(
            sent(&mut invoker),
            [
                (0, "speed".to_owned(), Data::Record(args(1))),
                (1, "reboot".to_owned(), Data::Record(vec![]))
            ]
        );
        assert_eq!(invoker.outcome("speed", 0.0), Some(&Outcome::Pending));

        invoker.receive(&answer(0, Ok(())), 1.0);
        invoker.receive(&answer(1, Err("busy")), 1.0);
        assert_eq!(invoker.outcome("speed", 2.0), Some(&Outcome::Accepted(1.0)));
        assert_eq!(invoker.outcome("speed", 1.0 + SHOW_ACCEPTED + 0.1), None);
        assert_eq!(
            invoker.outcome("reboot", 100.0),
            Some(&Outcome::Rejected("busy".to_owned()))
        );
        // unknown and repeated answers change nothing
        invoker.receive(&answer(1, Ok(())), 2.0);
        invoker.receive(&answer(7, Ok(())), 2.0);
        assert_eq!(
            invoker.outcome("reboot", 100.0),
            Some(&Outcome::Rejected("busy".to_owned()))
        );
    }

    #[test]
    fn only_the_latest_invocation_of_an_action_shows() {
        let mut invoker = Invoker::default();
        invoker.invoke("speed", args(1), 0.0);
        invoker.invoke("speed", args(2), 0.0);
        invoker.receive(&answer(0, Err("old")), 0.5);
        assert_eq!(invoker.outcome("speed", 0.5), Some(&Outcome::Pending));
        invoker.receive(&answer(1, Ok(())), 0.6);
        assert_eq!(invoker.outcome("speed", 0.6), Some(&Outcome::Accepted(0.6)));
    }

    #[test]
    fn unanswered_invocations_time_out() {
        let mut invoker = Invoker::default();
        invoker.invoke("speed", args(1), 0.0);
        invoker.update(ANSWER_TIMEOUT);
        assert_eq!(invoker.outcome("speed", 0.0), Some(&Outcome::Pending));
        invoker.update(ANSWER_TIMEOUT + 0.1);
        assert_eq!(invoker.outcome("speed", 0.0), Some(&Outcome::Unanswered));
        assert!(!invoker.is_waiting(ANSWER_TIMEOUT + 0.1));
        // a late answer changes nothing
        invoker.receive(&answer(0, Ok(())), 6.0);
        assert_eq!(invoker.outcome("speed", 6.0), Some(&Outcome::Unanswered));
    }

    #[test]
    fn dragging_sends_at_most_every_interval_and_the_last_value() {
        let mut invoker = Invoker::default();
        invoker.invoke_dragging("speed", args(1), 0.0);
        invoker.invoke_dragging("speed", args(2), 0.03);
        invoker.invoke_dragging("speed", args(3), 0.06);
        invoker.update(0.08);
        let values = |sent: Vec<(u32, String, Data)>| -> Vec<Data> {
            sent.into_iter().map(|(_, _, args)| args).collect()
        };
        assert_eq!(values(sent(&mut invoker)), [Data::Record(args(1))]);
        assert!(invoker.is_waiting(0.08));
        // the held back value is sent once the interval passed
        invoker.update(0.1);
        assert_eq!(values(sent(&mut invoker)), [Data::Record(args(3))]);
        invoker.update(0.5);
        assert!(sent(&mut invoker).is_empty());

        // releasing sends the final value right away, dropping held back ones
        invoker.invoke_dragging("speed", args(4), 0.5);
        invoker.invoke_dragging("speed", args(5), 0.52);
        invoker.finish_dragging("speed", args(6), 0.55);
        invoker.update(1.0);
        assert_eq!(
            values(sent(&mut invoker)),
            [Data::Record(args(4)), Data::Record(args(6))]
        );

        // releasing at the value sent last, or without dragging, sends nothing
        invoker.invoke_dragging("speed", args(7), 2.0);
        invoker.finish_dragging("speed", args(7), 2.05);
        invoker.finish_dragging("speed", args(8), 3.0);
        assert_eq!(values(sent(&mut invoker)), [Data::Record(args(7))]);
    }

    #[test]
    fn resetting_keeps_the_inputs() {
        let mut invoker = Invoker::default();
        invoker
            .inputs
            .insert(("speed".to_owned(), "rpm".to_owned()), Data::Uint(5));
        invoker.invoke("speed", args(1), 0.0);
        invoker.reset(false);
        assert!(invoker.take_outbox().is_empty());
        assert_eq!(invoker.outcome("speed", 0.0), None);
        assert_eq!(invoker.inputs.len(), 1);
    }
}
