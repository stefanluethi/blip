use crate::{Data, Sample};
use serde::de::{self, IgnoredAny, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A block of samples of one trace.
#[derive(Debug, Clone, PartialEq)]
pub struct Samples {
    pub path: String,
    pub values: Vec<Data>,
    pub times: SampleTimes,
}

/// When the samples of a block were taken, in nanoseconds.
#[derive(Debug, Clone, PartialEq)]
pub enum SampleTimes {
    /// Sample `i` at `timestamp + i * period`.
    Period { timestamp: u64, period: u64 },
    /// The timestamp of each sample.
    Each(Vec<u64>),
}

impl SampleTimes {
    /// The times of a block of `len` values from the fields of a `Samples` message, or
    /// why they do not make one.
    pub fn new(
        timestamp: Option<u64>,
        period: Option<u64>,
        timestamps: Option<Vec<u64>>,
        len: usize,
    ) -> Result<Self, String> {
        let times = match (timestamp, period, timestamps) {
            (None, None, Some(list)) => Self::Each(list),
            (_, _, Some(_)) => return Err("a Samples has both a period and timestamps".into()),
            (Some(timestamp), Some(period), None) => Self::Period { timestamp, period },
            (Some(_), None, None) => return Err("a Samples has a timestamp but no period".into()),
            (None, Some(_), None) => return Err("a Samples has a period but no timestamp".into()),
            (None, None, None) => {
                return Err("a Samples has neither a period nor timestamps".into());
            }
        };
        match &times {
            Self::Each(list) if list.len() != len => {
                return Err(format!(
                    "a Samples has {len} values but {} timestamps",
                    list.len()
                ));
            }
            Self::Period { .. } if len > 0 && times.timestamp(len - 1).is_none() => {
                return Err("the timestamps of a Samples go beyond 2^64 ns".into());
            }
            _ => {}
        }
        Ok(times)
    }

    /// The timestamp of sample `index`, `None` past `u64::MAX` or the list.
    pub fn timestamp(&self, index: usize) -> Option<u64> {
        match self {
            Self::Period { timestamp, period } => {
                timestamp.checked_add(period.checked_mul(u64::try_from(index).ok()?)?)
            }
            Self::Each(list) => list.get(index).copied(),
        }
    }
}

impl Samples {
    /// The samples of the block one by one, as single `Sample`s.
    pub fn to_samples(&self) -> Vec<Sample> {
        self.values
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                Some(Sample {
                    timestamp: self.times.timestamp(index)?,
                    path: self.path.clone(),
                    value: value.clone(),
                })
            })
            .collect()
    }
}

impl Serialize for Samples {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let len = match self.times {
            SampleTimes::Period { .. } => 4,
            SampleTimes::Each(_) => 3,
        };
        let mut map = serializer.serialize_map(Some(len))?;
        map.serialize_entry("path", &self.path)?;
        map.serialize_entry("values", &self.values)?;
        match &self.times {
            SampleTimes::Period { timestamp, period } => {
                map.serialize_entry("timestamp", timestamp)?;
                map.serialize_entry("period", period)?;
            }
            SampleTimes::Each(list) => map.serialize_entry("timestamps", list)?,
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Samples {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SamplesVisitor;

        impl<'de> Visitor<'de> for SamplesVisitor {
            type Value = Samples;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map of a block of samples")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Samples, A::Error> {
                let mut path = None;
                let mut values: Option<Vec<Data>> = None;
                let mut timestamp = None;
                let mut period = None;
                let mut timestamps = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "path" => once(&mut path, "path", &mut map)?,
                        "values" => once(&mut values, "values", &mut map)?,
                        "timestamp" => once(&mut timestamp, "timestamp", &mut map)?,
                        "period" => once(&mut period, "period", &mut map)?,
                        "timestamps" => once(&mut timestamps, "timestamps", &mut map)?,
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                let path = path.ok_or_else(|| de::Error::missing_field("path"))?;
                let values = values.ok_or_else(|| de::Error::missing_field("values"))?;
                let times = SampleTimes::new(timestamp, period, timestamps, values.len())
                    .map_err(de::Error::custom)?;
                Ok(Samples {
                    path,
                    values,
                    times,
                })
            }
        }

        deserializer.deserialize_map(SamplesVisitor)
    }
}

/// Reads the value of field `name` into `slot`, an error if it is there already.
fn once<'de, T: Deserialize<'de>, A: MapAccess<'de>>(
    slot: &mut Option<T>,
    name: &'static str,
    map: &mut A,
) -> Result<(), A::Error> {
    if slot.is_some() {
        return Err(de::Error::duplicate_field(name));
    }
    *slot = Some(map.next_value()?);
    Ok(())
}
