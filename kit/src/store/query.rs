//! The store as the widgets read it: the [`Store`] trait, the [`Window`] a widget asks
//! for and the [`Loaded`] answer.

use super::actions::Actions;
use super::logs::Logs;
use super::numbers::Labels;
use super::time_base::TimeBase;
use super::traces::{Series, Trace};
use super::{Activity, Version};

/// What a widget asks a [`Store`] for of the samples of a trace or of the log records:
/// a range of time and how fine. A store may answer with more than asked for: the
/// in-memory store always answers with all it has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Window {
    /// The timestamps asked for in nanoseconds, from the first up to the second.
    /// `None` asks for all there is. A store also gives the nearest sample before and
    /// after the range, if there is one, so that lines and states reach its edges.
    pub range: Option<(u64, u64)>,
    /// About how many samples across the range are of use, e.g. two per pixel of a
    /// plot. A store with more may thin them out, keeping the lowest and highest of
    /// what it leaves out. `usize::MAX` asks for every sample, 0 for none: only for
    /// what the trace is, its kind and shape.
    pub max_points: usize,
}

impl Window {
    /// Every sample there is.
    pub const ALL: Self = Self {
        range: None,
        max_points: usize::MAX,
    };

    /// No samples: what the trace is, its kind, its shape and the range of all its
    /// values, is enough.
    pub const NONE: Self = Self {
        range: None,
        max_points: 0,
    };

    /// Every sample from `start` up to `end` nanoseconds.
    pub fn between(start: u64, end: u64) -> Self {
        Self {
            range: Some((start, end)),
            max_points: usize::MAX,
        }
    }

    /// The samples around `timestamp` nanoseconds: the nearest before and after it,
    /// which a store gives with the range, e.g. for the value current at a time.
    pub fn at(timestamp: u64) -> Self {
        Self::between(timestamp, timestamp)
    }

    /// Every sample of `range`, all there is if `None`.
    pub fn of(range: Option<(u64, u64)>) -> Self {
        Self {
            range,
            max_points: usize::MAX,
        }
    }

    /// The same range with about `max_points` samples across it.
    pub fn points(self, max_points: usize) -> Self {
        Self { max_points, ..self }
    }
}

/// The answer of a [`Store`]: what it has now, and whether more is on its way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Loaded<T> {
    /// What the store has of what was asked for.
    pub data: T,
    /// Whether the store is still loading what was asked for. It asks for a repaint
    /// when more arrived; the widget then asks again.
    pub pending: bool,
}

impl<T> Loaded<T> {
    /// All of what was asked for.
    pub fn complete(data: T) -> Self {
        Self {
            data,
            pending: false,
        }
    }

    /// What the store has so far, with more on its way.
    pub fn partial(data: T) -> Self {
        Self {
            data,
            pending: true,
        }
    }
}

/// A store of what the targets sent, as the widgets read it. The in-memory store,
/// [`MemoryStore`](super::MemoryStore), is one; another may keep the data elsewhere,
/// e.g. in a database, and load what the widgets ask for.
///
/// The widgets ask each frame, so no call may block. A store that has to load what is
/// asked for answers with what it has, says so in [`Loaded::pending`], loads the rest
/// on its own (it takes `&self`, so it notes what was asked for behind a `Cell` or a
/// lock) and asks for a repaint when it arrived.
///
/// The answers borrow the columns of the store, so the in-memory store copies nothing.
/// A store that loads keeps what it loaded in the same types: [`Traces`](super::traces::Traces)
/// for the samples and [`Logs`] for the log records.
pub trait Store {
    /// Changes whenever a trace, the first log record of a path or a list of actions
    /// arrives, so widgets list the paths again only then.
    fn paths_version(&self) -> Version;

    /// Changes whenever a sample or a log record arrives, or more of what was asked
    /// for was loaded.
    fn samples_version(&self) -> Version;

    /// Whether the store is loading anything a widget asked for.
    fn pending(&self) -> bool {
        false
    }

    /// The timestamp the times of the widgets count from, of the first sample or log
    /// record.
    fn origin(&self) -> Option<u64>;

    /// What the timestamps count from, `None` while there is no data to tell.
    fn time_base(&self) -> Option<TimeBase>;

    /// The timestamps of the first and the newest sample or log record.
    fn time_span(&self) -> Option<(u64, u64)>;

    /// What arrived in each of `bins` bins of `step` nanoseconds from `start`
    /// nanoseconds on, for the timeline.
    fn activity(&self, start: u64, step: u64, bins: usize) -> Vec<Activity>;

    /// All traces, sorted by path so the fields of a record follow each other, each
    /// with the samples the store has at hand: what a trace is counts, its path, kind
    /// and shape.
    fn traces(&self) -> Box<dyn Iterator<Item = Trace<'_>> + '_>;

    /// Changes whenever the kind of a trace changes, e.g. with its first value, which
    /// changes what views can show of it.
    fn kinds(&self) -> u64;

    /// The trace at `path` with its samples in `window`.
    fn trace(&self, path: &str, window: Window) -> Loaded<Option<Trace<'_>>>;

    /// The numbers in `window` of the numeric trace at `path`, or of the element of an
    /// array trace at the path of the element, e.g. `imu.acc[0]`.
    fn series(&self, path: &str, window: Window) -> Loaded<Option<Series<'_>>>;

    /// The paths of the fields of the record at `path`, also of nested records.
    fn field_paths<'a>(&'a self, path: &str) -> Box<dyn Iterator<Item = &'a str> + 'a>;

    /// The metadata the target declared for the trace at `path`.
    fn metadata(&self, path: &str) -> Option<&proto::TraceMetadata>;

    /// The unit of the trace at `path`, of an array for its elements.
    fn unit(&self, path: &str) -> Option<&str>;

    /// The description of the trace at `path`, of an array for its elements.
    fn description(&self, path: &str) -> Option<&str>;

    /// The labels of the values of the trace at `path`, of an array for its elements.
    fn labels(&self, path: &str) -> Option<&Labels>;

    /// The range the values of the trace at `path` are expected in, of an array for
    /// its elements.
    fn expected(&self, path: &str) -> Option<(f64, f64)>;

    /// The log records in `window`, by path. With [`Window::NONE`] the paths count.
    fn logs(&self, window: Window) -> Loaded<&Logs>;

    /// The actions the targets announced.
    fn actions(&self) -> &Actions;

    /// The trace at `readback` with the latest sample only, if an announced action has
    /// it as its readback. It is there also while the samples are not kept.
    fn readback(&self, readback: &str) -> Option<Trace<'_>>;

    /// How many values were received at the path holding the readback trace at
    /// `readback`, 0 if there is none. It only grows.
    fn readback_samples(&self, readback: Option<&str>) -> usize;
}
