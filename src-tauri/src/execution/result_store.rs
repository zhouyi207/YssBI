use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResultId(u64);

impl ResultId {
    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StoredResult {
    Scalar(f64),
    Text(Box<str>),
    Empty,
}

pub struct ResultStore {
    values: RwLock<BTreeMap<ResultId, Arc<StoredResult>>>,
}

impl ResultStore {
    pub fn new() -> Self {
        Self {
            values: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn publish(&self, result: ResultId, value: StoredResult) -> Arc<StoredResult> {
        let value = Arc::new(value);
        self.values
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .insert(result, Arc::clone(&value));
        value
    }

    pub fn get(&self, result: ResultId) -> Option<Arc<StoredResult>> {
        self.values
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get(&result)
            .cloned()
    }

    pub fn clear(&self) {
        self.values
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
    }
}

impl Default for ResultStore {
    fn default() -> Self {
        Self::new()
    }
}
