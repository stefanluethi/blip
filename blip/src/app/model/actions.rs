//! The actions the targets announced, and the invocations blip sends to carry them out.

use super::Model;
use proto::{ActionDescriptor, ActionResult, Data, Invocation, Request};
use std::collections::{HashMap, HashSet};

/// Seconds after which an invocation without an answer shows as unanswered. A later
/// answer still counts, as the outpost answers only once the action returns.
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

    /// Whether the value at `path` holds the readback trace of an announced action:
    /// is it, or a record or array it is a field or element of.
    pub fn holds_readback(&self, path: &str) -> bool {
        self.list
            .iter()
            .filter_map(|action| action.descriptor.readback.as_deref())
            .any(|readback| holds(path, readback))
    }
}

/// Whether the value at `path` holds the trace at `trace`: is it, or a record or array
/// it is a field or element of, e.g. `motor` holds `motor.speed` and `adc` `adc[1]`.
pub fn holds(path: &str, trace: &str) -> bool {
    trace
        .strip_prefix(path)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['.', '[']))
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
    /// Not answered within [`ANSWER_TIMEOUT`], still waiting for the answer.
    Unanswered,
}

/// The latest invocation of an action.
#[derive(Clone, Debug, PartialEq)]
pub struct Latest {
    id: u32,
    pub outcome: Outcome,
    /// When it was sent.
    pub sent_at: f64,
    pub args: Args,
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
    /// Whether blip is recording: actions are invoked only then, so that the recording
    /// shows what they did.
    pub recording: bool,
    next_id: u32,
    /// The invocations without answer, by id.
    pending: HashMap<u32, Pending>,
    /// The latest invocation of each action.
    latest: HashMap<ActionId, Latest>,
    /// The arguments of the latest accepted invocation of each action, or until one is,
    /// those its inputs had before it was first invoked.
    accepted: HashMap<ActionId, Args>,
    /// The actions whose latest invocation was rejected since the view last took it
    /// with [`Self::take_rejected`].
    rejected: HashSet<ActionId>,
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
            args: Data::Record(args.clone()),
        };
        self.outbox
            .push((action.source.clone(), Request::Invoke(invocation)));
        let pending = Pending {
            action: action.clone(),
            readback: readback.map(str::to_owned),
            sent_at: now,
        };
        self.pending.insert(id, pending);
        let latest = Latest {
            id,
            outcome: Outcome::Pending,
            sent_at: now,
            args,
        };
        self.latest.insert(action.clone(), latest);
        self.rejected.remove(action);
    }

    /// Whether the input of `action` is dragged, from its first value sent while
    /// dragged until it is let go.
    pub fn is_dragging(&self, action: &ActionId) -> bool {
        self.dragged.contains_key(action)
    }

    /// Whether `action` can be invoked, as the connection of its outpost is open.
    pub fn is_connected(&self, action: &ActionId) -> bool {
        self.connected.contains(&action.source)
    }

    /// Takes the answer to an invocation received from the connection `source`, also
    /// after [`ANSWER_TIMEOUT`]. Answers from another connection than the one the
    /// invocation was sent to are ignored.
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
        let Some(latest) = self.latest.get_mut(&pending.action) else {
            return;
        };
        if latest.id != result.id {
            return;
        }
        match &result.result {
            Ok(()) => {
                latest.outcome = Outcome::Accepted(now);
                self.accepted
                    .insert(pending.action.clone(), latest.args.clone());
                let samples = model.readback_samples(pending.readback.as_deref());
                self.readback_marks.insert(pending.action, samples);
            }
            Err(reason) => {
                latest.outcome = Outcome::Rejected(reason.clone());
                self.rejected.insert(pending.action);
            }
        }
    }

    /// Whether an input of `action` shows the readback, given the number of samples of
    /// its readback trace: not while the latest invocation is pending, and once it
    /// was accepted only from a sample that arrived after the answer, as earlier ones
    /// may show the state before.
    pub fn follows_readback(&self, action: &ActionId, readback_samples: usize) -> bool {
        match self.latest.get(action).map(|latest| &latest.outcome) {
            Some(Outcome::Pending) => false,
            Some(Outcome::Accepted(_)) => self
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

        // the latest invocation of each action keeps waiting for a late answer, the
        // answers of earlier ones would change nothing
        let expired: Vec<u32> = self
            .pending
            .iter()
            .filter(|(_, pending)| now - pending.sent_at > ANSWER_TIMEOUT)
            .map(|(&id, _)| id)
            .collect();
        for id in expired {
            let action = &self.pending[&id].action;
            match self.latest.get_mut(action) {
                Some(latest) if latest.id == id => latest.outcome = Outcome::Unanswered,
                _ => {
                    self.pending.remove(&id);
                }
            }
        }
    }

    /// The outcome of the latest invocation of `action` to show, none once an accepted
    /// one was shown for [`SHOW_ACCEPTED`].
    pub fn outcome(&self, action: &ActionId, now: f64) -> Option<&Outcome> {
        match &self.latest.get(action)?.outcome {
            Outcome::Accepted(at) if now - at > SHOW_ACCEPTED => None,
            outcome => Some(outcome),
        }
    }

    /// The latest invocation of `action`, if it was invoked.
    pub fn latest(&self, action: &ActionId) -> Option<&Latest> {
        self.latest.get(action)
    }

    /// The arguments of the latest accepted invocation of `action`, or those its inputs
    /// had before it was first invoked, see [`Self::assume_accepted`].
    pub fn accepted(&self, action: &ActionId) -> Option<&Args> {
        self.accepted.get(action)
    }

    /// Takes `args`, what the inputs of `action` showed before they changed, as what
    /// the target holds until an invocation of it is accepted, so that a rejected value
    /// goes back to them and not, e.g., to a readback that is still settling.
    pub fn assume_accepted(&mut self, action: &ActionId, args: Args) {
        self.accepted.entry(action.clone()).or_insert(args);
    }

    /// Whether the latest invocation of `action` was rejected since the last call, so
    /// that its inputs go back to what the target accepted.
    pub fn take_rejected(&mut self, action: &ActionId) -> bool {
        self.rejected.remove(action)
    }

    /// The seconds after which blip repaints because the outcomes shown change in time
    /// without new messages, if they do: often while an answer is due, a held back
    /// value is to be sent or an accepted invocation shows, else each second to count
    /// the seconds without an answer.
    pub fn repaint_after(&self, now: f64) -> Option<f64> {
        let outcomes = || self.latest.values().map(|latest| &latest.outcome);
        let soon = outcomes().any(|outcome| match outcome {
            Outcome::Pending => true,
            Outcome::Accepted(at) => now - at <= SHOW_ACCEPTED,
            _ => false,
        }) || self.dragged.values().any(|dragged| dragged.held.is_some());
        if soon {
            Some(DRAG_INTERVAL)
        } else if outcomes().any(|outcome| *outcome == Outcome::Unanswered) {
            Some(1.0)
        } else {
            None
        }
    }

    /// The requests to send, each with the name of the connection to send it to.
    pub fn take_outbox(&mut self) -> Vec<(String, Request)> {
        std::mem::take(&mut self.outbox)
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
mod tests;
