use std::collections::HashMap;

// todo: any
#[derive(serde::Deserialize, serde::Serialize)]
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

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
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

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
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

    pub fn append(&mut self, value: &proto::Value) {
        if !self.map.contains_key(&value.path) {
            self.map.insert(value.path.clone(), Stream::new(value.path.clone()));
        }
        let stream = self.map.get_mut(&value.path).unwrap();
        stream.values.push(value.into());
    }

    pub fn clear(&mut self) {
        for stream in self.map.values_mut() {
            stream.values.clear();
        }
    }
}
