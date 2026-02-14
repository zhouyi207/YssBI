//! Node 定义（静态描述）

use crate::execution::{ExecutionEffect, NodeExecutionContextTrait};
use crate::graph::{PinDefinition, TypeVarDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type FlowProcessor =
    Arc<dyn Fn(&mut dyn NodeExecutionContextTrait) -> Result<ExecutionEffect, String> + Send + Sync>;

pub type DataEvaluator =
    Arc<dyn Fn(&mut dyn NodeExecutionContextTrait) -> Result<(), String> + Send + Sync>;

pub type PinGenerator =
    Arc<dyn Fn() -> Result<Vec<PinDefinition>, String> + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetaData {
    /// UI 样式
    pub ui_style: String,

    /// 描述
    #[serde(skip_serializing_if = "Option::is_none")]
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
    /// 节点标题（显示名称）
    pub name: String,

    /// 分类路径
    pub category: Vec<String>,

    /// 节点的 pin 类型推断
    pub type_vars: Vec<TypeVarDefinition>,

    /// 决定执行流向，返回下一个要触发的 ExecRole
    #[serde(skip)]
    pub flow_processor: Option<FlowProcessor>,

    /// 计算输出数据值，通过 DataRole 访问输入输出
    #[serde(skip)]
    pub data_evaluator: Option<DataEvaluator>,

    /// 动态生成输入输出 Pin
    #[serde(skip)]
    pub pin_generator: Option<PinGenerator>,

    /// 元数据
    pub metadata: NodeMetaData,
}

// 手动实现 Debug，因为函数指针不支持 Debug
impl std::fmt::Debug for NodeDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeDefinition")
            .field("name", &self.name)
            .field("category", &self.category)
            .field("type_vars", &self.type_vars)
            .field("flow_processor", &self.flow_processor.as_ref().map(|_| "<function>"))
            .field("data_evaluator", &self.data_evaluator.as_ref().map(|_| "<function>"))
            .field("pin_generator", &self.pin_generator.as_ref().map(|_| "<function>"))
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl NodeDefinition {
    /// 创建新的节点定义
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            category: vec![],
            type_vars: vec![],
            flow_processor: None,
            data_evaluator: None,
            pin_generator: None,
            metadata: NodeMetaData::default(),
        }
    }

    // 在定义时这样使用
    //     .add_type_var(TypeVarDefinition {
    //     id: TypeVarId::placeholder("T"),
    //     constraints: vec![TypeConstraint::Numeric],
    //     bound: None,
    // })
    pub fn with_type_vars(mut self, type_vars: Vec<TypeVarDefinition>) -> Self {
        self.type_vars = type_vars;
        self
    }

    /// 设置分类
    pub fn with_category(mut self, category: Vec<String>) -> Self {
        self.category = category;
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

    pub fn with_pin_generator(mut self, generator: PinGenerator) -> Self {
        self.pin_generator = Some(generator);
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
}
