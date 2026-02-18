//! Node 定义（静态描述）

use crate::execution::{ExecutionEffect, NodeExecutionContextTrait};
use crate::graph::{PinDefinition, TypeVarDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type FlowProcessor =
    Arc<dyn Fn(&mut dyn NodeExecutionContextTrait) -> Result<ExecutionEffect, String> + Send + Sync>;

pub type DataEvaluator =
    Arc<dyn Fn(&mut dyn NodeExecutionContextTrait) -> Result<(), String> + Send + Sync>;

/// 静态 Pin 生成器：节点创建时调用一次，生成初始 pins
pub type PinGenerator =
    Arc<dyn Fn() -> Result<Vec<PinDefinition>, String> + Send + Sync>;

/// 动态 Pin 解析器：连接/参数变化时重新调用，根据上下文动态生成 pins
pub type PinResolver =
    Arc<dyn Fn(&PinResolverContext) -> Result<Vec<PinDefinition>, String> + Send + Sync>;

/// Pin 解析器上下文
///
/// 提供节点的运行时信息（instance_params）和上游输入的 schema，
/// 用于 `PinResolver` 决定应该生成哪些 pins。
#[derive(Debug, Clone, Default)]
pub struct PinResolverContext {
    pub instance_params: super::NodeInstanceParams,
    pub input_schemas: std::collections::HashMap<crate::graph::PinRole, DataSchema>,
}

/// 数据源 schema（如 DataFrame 的列结构）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataSchema {
    pub columns: Vec<ColumnSchema>,
}

/// 数据列 schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: crate::graph::DataType,
}

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

    // 唯一定义
    pub node_type: String,

    /// 节点的 pin 类型推断
    pub type_vars: Vec<TypeVarDefinition>,

    /// 决定执行流向，返回下一个要触发的 ExecRole
    #[serde(skip)]
    pub flow_processor: Option<FlowProcessor>,

    /// 计算输出数据值，通过 DataRole 访问输入输出
    #[serde(skip)]
    pub data_evaluator: Option<DataEvaluator>,

    /// 静态生成输入输出 Pin（创建时调用一次）
    #[serde(skip)]
    pub pin_generator: Option<PinGenerator>,

    /// 动态 Pin 解析器（连接/参数变化时重新调用，可选）
    #[serde(skip)]
    pub pin_resolver: Option<PinResolver>,

    /// 元数据
    pub metadata: NodeMetaData,
}

// 手动实现 Debug，因为函数指针不支持 Debug
impl std::fmt::Debug for NodeDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeDefinition")
            .field("name", &self.name)
            .field("category", &self.category)
            .field("node_type", &self.node_type)
            .field("type_vars", &self.type_vars)
            .field("flow_processor", &self.flow_processor.as_ref().map(|_| "<function>"))
            .field("data_evaluator", &self.data_evaluator.as_ref().map(|_| "<function>"))
            .field("pin_generator", &self.pin_generator.as_ref().map(|_| "<function>"))
            .field("pin_resolver", &self.pin_resolver.as_ref().map(|_| "<function>"))
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl NodeDefinition {
    /// 创建新的节点定义
    pub fn new(name: impl Into<String>, category: Vec<String>) -> Self {
        let name = name.into();
        let mut category = category.clone();
        category.push(name.clone());
        let node_type = category.join(":");
        
        Self {
            name,
            category,
            node_type,
            type_vars: vec![],
            flow_processor: None,
            data_evaluator: None,
            pin_generator: None,
            pin_resolver: None,
            metadata: NodeMetaData::default(),
        }
    }

    /// 覆盖默认的 node_type（用于 get_variable、call_function 等前端约定的类型名）
    pub fn with_node_type(mut self, node_type: impl Into<String>) -> Self {
        self.node_type = node_type.into();
        self
    }

    pub fn with_type_vars(mut self, type_vars: Vec<TypeVarDefinition>) -> Self {
        self.type_vars = type_vars;
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

    /// 设置动态 Pin 解析器
    ///
    /// 有 `pin_resolver` 的节点在连接/参数变化时会重新计算 pins。
    /// `pin_generator` 仍然负责创建时的初始 pins。
    pub fn with_pin_resolver(mut self, resolver: PinResolver) -> Self {
        self.pin_resolver = Some(resolver);
        self.metadata.supports_dynamic_pins = true;
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
