//! Node 定义（静态描述）

use super::NodeProcessor;
use crate::executor::pin::PinDefinition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetaData {
    /// UI 样式
    pub ui_style: String,

    /// 描述
    pub description: Option<String>,

    /// 是否支持动态 Pin
    pub supports_dynamic_pins: bool,
}

impl Default for NodeMetaData {
    fn default() -> Self {
        Self {
            ui_style: "default".to_string(),
            description: None,
            supports_dynamic_pins: false,
        }
    }
}

/// Node 定义（静态描述，用于注册中心）
#[derive(Clone, Serialize, Deserialize)]
pub struct NodeDefinition {
    /// 节点类型（唯一标识符）
    pub node_type: String,

    /// 节点标题（显示名称）
    pub title: String,

    /// 分类路径
    pub category: Vec<String>,

    /// Pin 定义列表
    pub pins: Vec<PinDefinition>,

    /// 处理器（不可序列化，运行时设置）
    #[serde(skip)]
    pub processor: Option<NodeProcessor>,

    pub metadata: NodeMetaData,
}

impl std::fmt::Debug for NodeDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeDefinition")
            .field("node_type", &self.node_type)
            .field("title", &self.title)
            .field("category", &self.category)
            .field("pins", &self.pins)
            .finish()
    }
}

impl NodeDefinition {
    /// 创建新的节点定义
    pub fn new(node_type: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            node_type: node_type.into(),
            title: title.into(),
            category: vec![],
            pins: vec![],
            processor: None,
            metadata: NodeMetaData::default(),
        }
    }

    /// 添加 Pin 定义
    pub fn add_pin(mut self, pin: PinDefinition) -> Self {
        self.pins.push(pin);
        self
    }

    /// 设置分类
    pub fn with_category(mut self, category: Vec<String>) -> Self {
        self.category = category;
        self
    }

    /// 设置 UI 样式
    pub fn with_ui_style(mut self, style: impl Into<String>) -> Self {
        self.metadata.ui_style = style.into();
        self
    }

    /// 设置描述
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.metadata.description = Some(desc.into());
        self
    }

    /// 设置处理器
    pub fn with_processor(mut self, processor: NodeProcessor) -> Self {
        self.processor = Some(processor);
        self
    }

    /// 启用动态 Pin
    pub fn dynamic(mut self) -> Self {
        self.metadata.supports_dynamic_pins = true;
        self
    }
}
