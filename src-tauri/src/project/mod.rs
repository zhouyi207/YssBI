//! 项目文件模块
//!
//! 处理项目文件的序列化、反序列化、保存和加载。

pub mod io;

use crate::schema::VariableDefinition;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ==================== Pin 定义 ====================

/// Pin 定义（用于函数/宏的输入输出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinDefinition {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    #[serde(rename = "isArray", default)]
    pub is_array: bool,
}

// ==================== 节点数据 ====================

/// 序列化的 Pin 数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedPin {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(rename = "defaultValue", skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    #[serde(rename = "isArray", default)]
    pub is_array: bool,
}

/// 序列化的节点数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    pub position: Position,

    #[serde(rename = "isInternal", default)]
    pub is_internal: bool,

    // 变量节点相关字段
    #[serde(rename = "variableId", skip_serializing_if = "Option::is_none")]
    pub variable_id: Option<String>,
    #[serde(rename = "variableType", skip_serializing_if = "Option::is_none")]
    pub variable_type: Option<String>,
    #[serde(rename = "variableName", skip_serializing_if = "Option::is_none")]
    pub variable_name: Option<String>,

    // 子图节点相关字段
    #[serde(rename = "subGraphId", skip_serializing_if = "Option::is_none")]
    pub sub_graph_id: Option<String>,

    #[serde(default)]
    pub inputs: Vec<SerializedPin>,
    #[serde(default)]
    pub outputs: Vec<SerializedPin>,
}

/// 位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// 画布状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CanvasState {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default = "default_scale")]
    pub scale: f64,
}

fn default_scale() -> f64 {
    1.0
}

// ==================== 子图数据 ====================

/// 子图类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SubGraphType {
    Event,
    Function,
    Macro,
}

/// 子图数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubGraphData {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub sub_type: SubGraphType,
    #[serde(default)]
    pub nodes: Vec<SerializedNode>,
    #[serde(default)]
    pub canvas: CanvasState,
    /// 局部变量
    #[serde(default)]
    pub variables: HashMap<String, VariableDefinition>,
    /// 函数/宏的输入参数
    #[serde(default)]
    pub inputs: Vec<PinDefinition>,
    /// 函数/宏的输出参数
    #[serde(default)]
    pub outputs: Vec<PinDefinition>,
}

// ==================== 数据帧数据 ====================

/// 数据帧列定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFrameColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: String,
}

/// 数据帧数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFrameData {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub columns: Vec<DataFrameColumn>,
    #[serde(default)]
    pub rows: Vec<Vec<serde_json::Value>>,
    #[serde(rename = "rowCount", default)]
    pub row_count: usize,
    #[serde(rename = "columnCount", default)]
    pub column_count: usize,
    #[serde(rename = "sourcePath", skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

// ==================== 项目数据 ====================

/// 项目元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    #[serde(rename = "exportTime")]
    pub export_time: String,
    #[serde(rename = "appVersion")]
    pub app_version: String,
}

impl Default for ProjectMetadata {
    fn default() -> Self {
        Self {
            export_time: chrono::Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// 项目数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    /// 全局变量
    #[serde(rename = "globalVariables", default)]
    pub global_variables: HashMap<String, VariableDefinition>,
    /// 事件子图
    #[serde(default)]
    pub events: HashMap<String, SubGraphData>,
    /// 函数子图
    #[serde(default)]
    pub functions: HashMap<String, SubGraphData>,
    /// 宏子图
    #[serde(default)]
    pub macros: HashMap<String, SubGraphData>,
    /// 数据帧
    #[serde(default)]
    pub dataframes: HashMap<String, DataFrameData>,
    /// 项目元数据
    #[serde(default)]
    pub metadata: ProjectMetadata,
}

impl Default for ProjectData {
    fn default() -> Self {
        Self {
            global_variables: HashMap::new(),
            events: HashMap::new(),
            functions: HashMap::new(),
            macros: HashMap::new(),
            dataframes: HashMap::new(),
            metadata: ProjectMetadata::default(),
        }
    }
}

impl ProjectData {
    /// 创建新的空项目
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 JSON 字符串解析项目
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Failed to parse project: {}", e))
    }

    /// 序列化为 JSON 字符串
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize project: {}", e))
    }

    /// 更新元数据时间戳
    pub fn update_metadata(&mut self) {
        self.metadata.export_time = chrono::Utc::now().to_rfc3339();
    }
}

// ==================== 文件操作 ====================

/// 保存项目到文件
pub fn save_project_to_file(project: &ProjectData, path: &str) -> Result<(), String> {
    let json = project.to_json()?;
    std::fs::write(path, json).map_err(|e| format!("Failed to write file: {}", e))
}

/// 从文件加载项目
pub fn load_project_from_file(path: &str) -> Result<ProjectData, String> {
    if !Path::new(path).exists() {
        return Err(format!("File not found: {}", path));
    }
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    ProjectData::from_json(&content)
}

// ==================== 测试 ====================
// 测试已移动到 tests/project_tests.rs
