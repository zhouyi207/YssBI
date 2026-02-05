//! Node 定义（静态描述）

use super::{DataEvaluator, FlowProcessor, NodeExecutionModel};
use crate::graph::{pin::PinDefinition, TypeVarDefinition};
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
///
/// 采用三层 Processor 模型：
/// 1. FlowProcessor - 控制流决策
/// 2. DataEvaluator - 数据求值
/// 3. Role → PinId 映射 - 由 Graph 层管理
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

    /// 节点的 pin 类型推断
    pub type_vars: Vec<TypeVarDefinition>,

    /// 🧱 第一层：控制流处理器（可选）
    /// 决定执行流向，返回下一个要触发的 ExecRole
    #[serde(skip)]
    pub flow_processor: Option<FlowProcessor>,

    /// 🧱 第二层：数据求值器（可选）
    /// 计算输出数据值，通过 DataRole 访问输入输出
    #[serde(skip)]
    pub data_evaluator: Option<DataEvaluator>,

    /// 元数据
    pub metadata: NodeMetaData,
}

impl std::fmt::Debug for NodeDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeDefinition")
            .field("node_type", &self.node_type)
            .field("title", &self.title)
            .field("category", &self.category)
            .field("pins", &self.pins)
            .field("has_flow_processor", &self.flow_processor.is_some())
            .field("has_data_evaluator", &self.data_evaluator.is_some())
            .field("execution_model", &self.execution_model())
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
            type_vars: vec![],
            flow_processor: None,
            data_evaluator: None,
            metadata: NodeMetaData::default(),
        }
    }

    /// 添加 Pin 定义
    pub fn add_pin(mut self, pin: PinDefinition) -> Self {
        self.pins.push(pin);
        self
    }

    /// 添加类型变量定义
    pub fn add_type_var(mut self, type_var: TypeVarDefinition) -> Self {
        self.type_vars.push(type_var);
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

    /// 设置控制流处理器
    pub fn with_flow_processor(mut self, processor: FlowProcessor) -> Self {
        self.flow_processor = Some(processor);
        self
    }

    /// 设置数据求值器
    pub fn with_data_evaluator(mut self, evaluator: DataEvaluator) -> Self {
        self.data_evaluator = Some(evaluator);
        self
    }

    /// 启用动态 Pin
    pub fn dynamic(mut self) -> Self {
        self.metadata.supports_dynamic_pins = true;
        self
    }

    /// 获取节点的执行模型
    pub fn execution_model(&self) -> NodeExecutionModel {
        NodeExecutionModel::infer(
            self.flow_processor.is_some(),
            self.data_evaluator.is_some(),
        )
    }

    /// 检查是否有控制流处理器
    pub fn has_flow_processor(&self) -> bool {
        self.flow_processor.is_some()
    }

    /// 检查是否有数据求值器
    pub fn has_data_evaluator(&self) -> bool {
        self.data_evaluator.is_some()
    }
}