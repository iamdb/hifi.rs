pub mod app;

use self::app::AppKey;
use gst::{ClockTime, State as GstState};
use gstreamer as gst;
use serde::{Deserialize, Serialize};
use sled::{IVec, Tree};
use std::fmt::Display;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct HifiDB(sled::Db);

impl HifiDB {
    pub fn open_tree(&self, name: &'static str) -> StateTree {
        StateTree::new(
            self.0
                .open_tree(name)
                .unwrap_or_else(|_| panic!("failed to open tree {}", name)),
        )
    }
}

#[derive(Debug, Clone)]
pub struct StateTree {
    db: Tree,
}

impl StateTree {
    pub fn new(db: Tree) -> StateTree {
        StateTree { db }
    }
    pub fn clear(&self) {
        self.db.clear().expect("failed to clear tree");
    }
    pub fn insert<K, T>(&self, key: AppKey, value: T)
    where
        K: FromStr,
        T: Serialize,
    {
        if let Ok(serialized) = bincode::serialize(&value) {
            self.db.insert(key.as_str(), serialized).unwrap();
        }
    }
    pub fn get<'a, K, T>(&self, key: AppKey) -> Option<T>
    where
        K: FromStr,
        T: Into<T> + From<IVec> + Deserialize<'a>,
    {
        if let Ok(record) = self.db.get(key.as_str()) {
            record.map(|value| value.into())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringValue(String);

impl From<IVec> for StringValue {
    fn from(ivec: IVec) -> Self {
        let deserialized: StringValue =
            bincode::deserialize(&ivec).expect("failed to deserialize status value");

        deserialized
    }
}

impl From<String> for StringValue {
    fn from(string: String) -> Self {
        StringValue(string)
    }
}

impl Display for StringValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl StringValue {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusValue(GstState);

impl From<IVec> for StatusValue {
    fn from(ivec: IVec) -> Self {
        let deserialized: StatusValue =
            bincode::deserialize(&ivec).expect("failed to deserialize status value");

        deserialized
    }
}

impl From<GstState> for StatusValue {
    fn from(state: GstState) -> Self {
        StatusValue(state)
    }
}

impl From<StatusValue> for GstState {
    fn from(state: StatusValue) -> Self {
        state.0
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, PartialOrd, Deserialize)]
pub struct ClockValue(ClockTime);

impl ClockValue {
    pub fn clock_time(&self) -> ClockTime {
        self.0
    }
}

impl From<IVec> for ClockValue {
    fn from(ivec: IVec) -> Self {
        let deserialized: ClockValue =
            bincode::deserialize(&ivec).expect("failed to deserialize status value");

        deserialized
    }
}

impl From<ClockTime> for ClockValue {
    fn from(time: ClockTime) -> Self {
        ClockValue(time)
    }
}

impl Display for ClockValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.to_string().as_str())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, PartialOrd, Deserialize)]
pub struct FloatValue(f64);

impl From<IVec> for FloatValue {
    fn from(ivec: IVec) -> Self {
        let deserialized: FloatValue =
            bincode::deserialize(&ivec).expect("failed to deserialize status value");

        deserialized
    }
}

impl From<f64> for FloatValue {
    fn from(float: f64) -> Self {
        FloatValue(float)
    }
}

impl From<FloatValue> for f64 {
    fn from(float: FloatValue) -> Self {
        float.0
    }
}
