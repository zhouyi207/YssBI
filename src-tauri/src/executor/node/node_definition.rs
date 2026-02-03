//! Node 定义（静态描述）

use super::NodeProcessor;
use crate::executor::pin::PinDefinition;
use serde::{Deserialize, Serialize};

/// Node 定义（静态描述，用于注册中心）
#[derive(Clone, Serialize, Deserialize)]
pub struct NodeDefinition {
    /// 节点类型（唯一标识符）
    pub node_type: String,
    
    /// 节点标题（显示名称）
    pub title: String,
    
    /// 分类路径
    pub category: Vec<String>,
    
    /// UI 样式
    pub ui_style: String,
    
    /// 描述
    pub description: Option<String>,
    
    /// Pin 定义列表
    pub pins: Vec<PinDefinition>,
    
    /// 处理器（不可序列化，运行时设置）
    #[serde(skip)]
    pub processor: Option<NodeProcessor>,
    
    /// 是否支持动态 Pin
    pub supports_dynamic_pins: bool,
    
    /// 动态 Pin 配置
    pub dynamic_pin_config: Option<DynamicPinConfig>,
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
            ui_style: "default".to_string(),
            description: None,
            pins: vec![],
            processor: None,
            supports_dynamic_pins: false,
            dynamic_pin_config: None,
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
        self.ui_style = style.into();
        self
    }

    /// 设置描述
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// 设置处理器
    pub fn with_processor(mut self, processor: NodeProcessor) -> Self {
        self.processor = Some(processor);
        self
    }

    /// 启用动态 Pin
    pub fn with_dynamic_pins(mut self, config: DynamicPinConfig) -> Self {
        self.supports_dynamic_pins = true;
        self.dynamic_pin_config = Some(config);
        self
    }
}

/// 动态 Pin 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicPinConfig {
    /// 最小 Pin 数量
    pub min_count: usize,
    
    /// 最大 Pin 数量
    pub max_count: Option<usize>,
    
    /// 是否可重新排序
    pub can_reorder: bool,
    
    /// Pin 名称模板（如 "Input {}"）
    pub name_template: String,
}

impl DynamicPinConfig {
    pub fn new(min_count: usize, max_count: Option<usize>) -> Self {
        Self {
            min_count,
            max_count,
            can_reorder: true,
            name_template: "Pin {}".to_string(),
        }
    }

    pub fn with_template(mut self, template: impl Into<String>) -> Self {
        self.name_template = template.into();
        self
    }
}
