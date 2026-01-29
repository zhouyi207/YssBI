//! 项目状态核心数据结构

use polars::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::project::ProjectData;

/// 全局项目状态
pub struct ProjectState {
    /// 项目数据
    pub data: Arc<RwLock<ProjectData>>,
    /// 当前项目文件路径
    pub current_path: Arc<RwLock<Option<String>>>,
    /// Polars 数据帧存储 (内存中)
    pub df_store: Arc<RwLock<HashMap<String, DataFrame>>>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectState {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(ProjectData::default())),
            current_path: Arc::new(RwLock::new(None)),
            df_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取项目数据的克隆
    pub fn get_data(&self) -> ProjectData {
        self.data.read().unwrap().clone()
    }

    /// 设置项目数据
    pub fn set_data(&self, data: ProjectData) {
        use tauri_plugin_log::log::info;
        info!(
            "[ProjectState] Setting data: global_vars={}, events={}, functions={}, macros={}, dataframes={}",
            data.global_variables.len(),
            data.events.len(),
            data.functions.len(),
            data.macros.len(),
            data.dataframes.len()
        );
        *self.data.write().unwrap() = data;

        // 关键修复：设置新项目数据时，清空内存中的 DataFrame 存储
        // 实际的 DataFrame 会在需要时重新加载或通过 import 命令重新添加
        self.df_store.write().unwrap().clear();
    }

    /// 获取当前路径
    pub fn get_current_path(&self) -> Option<String> {
        self.current_path.read().unwrap().clone()
    }

    /// 设置当前路径
    pub fn set_current_path(&self, path: Option<String>) {
        *self.current_path.write().unwrap() = path;
    }

    /// 清空项目
    pub fn clear(&self) {
        *self.data.write().unwrap() = ProjectData::default();
        *self.current_path.write().unwrap() = None;
        // 关键修复：清空项目时也清空内存中的 DataFrame
        self.df_store.write().unwrap().clear();
    }
}
