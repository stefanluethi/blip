use std::collections::HashMap;

// todo: any
pub struct Value {
    pub timestamp: u64,
    pub value: f32,
}

impl Value {
    /// The sample `value` if it is a number or null, which is stored as NaN.
    fn from_proto(value: &proto::Value) -> Option<Self> {
        let number = match value.value {
            proto::Data::Null => f32::NAN,
            proto::Data::Uint(number) => number as f32,
            proto::Data::Int(number) => number as f32,
            proto::Data::Float(number) => number as f32,
            _ => return None,
        };
        Some(Self {
            timestamp: value.timestamp,
            value: number,
        })
    }
}

pub struct Stream {
    pub path: String,
    pub values: Vec<Value>,
}

impl Default for Stream {
    fn default() -> Self {
        Self::new("none".to_string())
    }
}

impl Stream {
    pub fn new(path: String) -> Self {
        Self {
            path,
            values: vec![],
        }
    }
}

pub struct Streams {
    map: HashMap<String, Stream>,
}

impl Default for Streams {
    fn default() -> Self {
        Self::new()
    }
}

impl Streams {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Stream> {
        self.map.values()
    }

    pub fn get(&self, path: &str) -> Option<&Stream> {
        self.map.get(path)
    }

    pub fn append(&mut self, value: &proto::Value) {
        let Some(sample) = Value::from_proto(value) else {
            log::warn!("{}: only numbers are supported yet", value.path);
            return;
        };
        self.map
            .entry(value.path.clone())
            .or_insert_with(|| Stream::new(value.path.clone()))
            .values
            .push(sample);
    }

    /// Timestamps of the first and the newest value of all streams.
    pub fn time_span(&self) -> Option<(u64, u64)> {
        self.map
            .values()
            // values are appended in arrival order, so timestamps are ascending
            .filter_map(|stream| Some((stream.values.first()?.timestamp, stream.values.last()?.timestamp)))
            .reduce(|(first_a, last_a), (first_b, last_b)| (first_a.min(first_b), last_a.max(last_b)))
    }
}
