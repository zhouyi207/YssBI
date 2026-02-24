use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 临时存储新窗口所需数据，前端窗口准备好后通过 command 拉取。
/// 内部使用 Arc 以支持在 Executor 和 Tauri managed state 之间共享。
#[derive(Clone)]
pub struct WindowDataStore {
    data: Arc<Mutex<HashMap<String, String>>>,
}

impl WindowDataStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn insert(&self, key: String, value: String) {
        self.data.lock().unwrap().insert(key, value);
    }

    /// 非破坏性读取（兼容 React Strict Mode 双重挂载）
    pub fn get(&self, key: &str) -> Option<String> {
        self.data.lock().unwrap().get(key).cloned()
    }

    pub fn remove(&self, key: &str) {
        self.data.lock().unwrap().remove(key);
    }
}
