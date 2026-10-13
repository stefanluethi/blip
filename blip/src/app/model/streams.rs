use std::collections::HashMap;

// todo: any
pub struct Value {
    pub timestamp: u64,
    pub value: f32,
}

impl From<&proto::Value> for Value {
    fn from(value: &proto::Value) -> Self {
        Self {
            timestamp: value.timestamp,
            value: value.value,
        }
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
        if !self.map.contains_key(&value.path) {
            self.map.insert(value.path.clone(), Stream::new(value.path.clone()));
        }
        let stream = self.map.get_mut(&value.path).unwrap();
        stream.values.push(value.into());
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
