//! Node 定义（静态描述）

use crate::execution::{ExecutionEffect, NodeExecutionContextTrait};
use crate::graph::{PinDefinition, PinSlot, PinTypeCapability, TypeVarDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type FlowProcessor =
    Arc<dyn Fn(&mut dyn NodeExecutionContextTrait) -> Result<ExecutionEffect, String> + Send + Sync>;

pub type DataEvaluator =
    Arc<dyn Fn(&mut dyn NodeExecutionContextTrait) -> Result<(), String> + Send + Sync>;

/// 动态 Pin 解析器：连接/参数变化时重新调用，根据上下文动态生成 pins
///
/// 仅用于 `PinSlot::DerivedFromInput` 槽位的运行时解析，
/// 只需返回动态派生的 pins（不含 Fixed/Repeatable 的静态 pins）。
pub type PinResolver =
    Arc<dyn Fn(&PinResolverContext) -> Result<Vec<PinDefinition>, String> + Send + Sync>;

/// Pin 解析器上下文
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
    pub ui_style: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

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
/// 采用声明式 PinSlot 模型描述节点的 pin 接口：
/// - `Fixed`: 固定 pin
/// - `Repeatable`: 可增删的同类型 pin
/// - `DerivedFromInput`: 从上游 schema 派生的动态 pin
#[derive(Clone, Serialize, Deserialize)]
pub struct NodeDefinition {
    pub name: String,
    pub category: Vec<String>,
    pub node_type: String,

    /// 节点的类型变量（泛型推断）
    pub type_vars: Vec<TypeVarDefinition>,

    /// 声明式 pin 槽位定义（取代旧的 pin_generator 闭包）
    pub pin_slots: Vec<PinSlot>,

    /// 决定执行流向，返回下一个要触发的 ExecRole
    #[serde(skip)]
    pub flow_processor: Option<FlowProcessor>,

    /// 计算输出数据值，通过 DataRole 访问输入输出
    #[serde(skip)]
    pub data_evaluator: Option<DataEvaluator>,

    /// 动态 Pin 解析器（仅 DerivedFromInput 槽位需要，可选）
    #[serde(skip)]
    pub pin_resolver: Option<PinResolver>,

    pub metadata: NodeMetaData,
}

impl std::fmt::Debug for NodeDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeDefinition")
            .field("name", &self.name)
            .field("category", &self.category)
            .field("node_type", &self.node_type)
            .field("type_vars", &self.type_vars)
            .field("pin_slots", &self.pin_slots)
            .field("flow_processor", &self.flow_processor.as_ref().map(|_| "<function>"))
            .field("data_evaluator", &self.data_evaluator.as_ref().map(|_| "<function>"))
            .field("pin_resolver", &self.pin_resolver.as_ref().map(|_| "<function>"))
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl NodeDefinition {
    pub fn new(name: impl Into<String>, category: Vec<String>) -> Self {
        let name = name.into();
        let node_type = category.join(":");

        Self {
            name,
            category,
            node_type,
            type_vars: vec![],
            pin_slots: vec![],
            flow_processor: None,
            data_evaluator: None,
            pin_resolver: None,
            metadata: NodeMetaData::default(),
        }
    }

    pub fn with_node_type(mut self, node_type: impl Into<String>) -> Self {
        self.node_type = node_type.into();
        self
    }

    pub fn with_type_vars(mut self, type_vars: Vec<TypeVarDefinition>) -> Self {
        self.type_vars = type_vars;
        self
    }

    /// 设置声明式 pin 槽位
    pub fn with_pin_slots(mut self, slots: Vec<PinSlot>) -> Self {
        self.metadata.supports_dynamic_pins = slots.iter().any(|s| s.is_dynamic());
        self.pin_slots = slots;
        self
    }

    pub fn with_flow_processor(mut self, processor: FlowProcessor) -> Self {
        self.flow_processor = Some(processor);
        self
    }

    pub fn with_data_evaluator(mut self, evaluator: DataEvaluator) -> Self {
        self.data_evaluator = Some(evaluator);
        self
    }

    /// 设置动态 Pin 解析器（仅 DerivedFromInput 槽位需要）
    pub fn with_pin_resolver(mut self, resolver: PinResolver) -> Self {
        self.pin_resolver = Some(resolver);
        self
    }

    pub fn with_ui_style(mut self, style: impl Into<String>) -> Self {
        self.metadata.ui_style = style.into();
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.metadata.description = Some(desc.into());
        self
    }

    // ======================== Pin 生成 ========================

    /// 从 pin_slots 生成初始 pin 定义列表
    ///
    /// 替代旧的 `pin_generator()` 调用
    pub fn generate_initial_pins(&self) -> Result<Vec<PinDefinition>, String> {
        let mut pins = Vec::new();
        for slot in &self.pin_slots {
            pins.extend(slot.generate_initial_pins());
        }
        Ok(pins)
    }

    // ======================== 类型能力查询 ========================

    /// 获取该节点所有槽位的类型能力（含 Fixed、Repeatable、DerivedFromInput）
    ///
    /// 用于前端拖 pin 时过滤兼容节点
    pub fn type_capabilities(&self) -> Vec<PinTypeCapability> {
        self.pin_slots
            .iter()
            .flat_map(|slot| slot.type_capabilities())
            .collect()
    }
}
