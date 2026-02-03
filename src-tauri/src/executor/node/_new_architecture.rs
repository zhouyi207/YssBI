//! 新架构核心数据结构
//!
//! 按照 fix_node.md 和 fix_pin.md 的要求重构节点系统
//! 
//! 核心原则：
//! 1. Node 不持有 Pin
//! 2. Pin 不属于 Node
//! 3. Graph 是唯一的运行时真实世界（Single Source of Truth）
//! 4. Pin 通过语义角色（PinRole）访问，不通过 index 或 name
//! 5. NodeDefinition 是静态描述，NodeInstance 是运行时实例

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::executor::value::PinTypeDesc;
use crate::executor::types::DataValue;
use crate::executor::node::types::NodeState;

// ==================== 类型别名 ====================

pub type NodeId = Uuid;
pub type PinId = Uuid;
pub type NodeDefinitionId = String;

// ==================== Pin 角色系统 ====================

/// Pin 角色 - 语义标识符
/// 
/// Pin 的逻辑绑定必须通过语义角色完成，而不是 Pin 名称或索引
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinRole {
    // ===== 控制流角色 =====
    /// 主执行输入
    ExecIn,
    /// 主执行输出
    ExecOut,
    /// 条件分支 - True 路径
    ExecTrue,
    /// 条件分支 - False 路径
    ExecFalse,
    /// 循环体执行
    ExecLoopBody,
    /// 循环完成
    ExecLoopComplete,
    
    // ===== 数据角色 =====
    /// 条件值
    Condition,
    /// 主输入值
    Input,
    /// 主输出值
    Output,
    /// 结果值
    Result,
    /// 错误信息
    Error,
    
    // ===== 动态角色组 =====
    /// 操作数组（如 Add 的多个输入）
    Operands,
    /// 序列步骤（如 Sequence 的多个执行输出）
    Steps,
    /// 分支情况（如 Switch 的多个分支）
    Cases,
    /// 数组元素
    Elements,
    
    // ===== 特殊角色 =====
    /// 变量引用
    VariableRef,
    /// 索引
    Index,
    /// 键
    Key,
    /// 值
    Value,
    
    // ===== 自定义角色 =====
    /// 自定义语义角色
    Custom(String),
}

/// Pin 方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinDirection {
    Input,
    Output,
}

/// Pin 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinKind {
    /// 数据 Pin
    Data,
    /// 执行 Pin
    Exec,
}

/// Pin 定义（静态描述）
/// 
/// 定义 Pin 的语义角色、类型和约束，不包含运行时状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinDefinition {
    /// 语义角色
    pub role: PinRole,
    /// Pin 方向
    pub direction: PinDirection,
    /// Pin 类型（Data/Exec）
    pub kind: PinKind,
    /// 数据类型描述（仅 Data Pin）
    pub type_desc: Option<PinTypeDesc>,
    /// 显示名称（仅用于 UI/Debug）
    pub display_name: String,
    /// 是否属于动态组
    pub is_dynamic: bool,
    /// 动态组 ID（如果属于动态组）
    pub group_id: Option<String>,
}

impl PinDefinition {
    /// 创建数据输入 Pin 定义
    pub fn data_input(role: PinRole, display_name: impl Into<String>, type_desc: PinTypeDesc) -> Self {
        Self {
            role,
            direction: PinDirection::Input,
            kind: PinKind::Data,
            type_desc: Some(type_desc),
            display_name: display_name.into(),
            is_dynamic: false,
            group_id: None,
        }
    }
    
    /// 创建数据输出 Pin 定义
    pub fn data_output(role: PinRole, display_name: impl Into<String>, type_desc: PinTypeDesc) -> Self {
        Self {
            role,
            direction: PinDirection::Output,
            kind: PinKind::Data,
            type_desc: Some(type_desc),
            display_name: display_name.into(),
            is_dynamic: false,
            group_id: None,
        }
    }
    
    /// 创建执行输入 Pin 定义
    pub fn exec_input(role: PinRole, display_name: impl Into<String>) -> Self {
        Self {
            role,
            direction: PinDirection::Input,
            kind: PinKind::Exec,
            type_desc: None,
            display_name: display_name.into(),
            is_dynamic: false,
            group_id: None,
        }
    }
    
    /// 创建执行输出 Pin 定义
    pub fn exec_output(role: PinRole, display_name: impl Into<String>) -> Self {
        Self {
            role,
            direction: PinDirection::Output,
            kind: PinKind::Exec,
            type_desc: None,
            display_name: display_name.into(),
            is_dynamic: false,
            group_id: None,
        }
    }
    
    /// 创建动态组 Pin 定义
    pub fn dynamic_group(
        role: PinRole,
        direction: PinDirection,
        kind: PinKind,
        type_desc: Option<PinTypeDesc>,
        group_id: impl Into<String>,
    ) -> Self {
        Self {
            role,
            direction,
            kind,
            type_desc,
            display_name: String::new(), // 动态生成
            is_dynamic: true,
            group_id: Some(group_id.into()),
        }
    }
}

// ==================== 节点定义（静态） ====================

/// 节点处理器类型
pub type NodeProcessor = Arc<dyn Fn(&mut dyn NodeExecutionContext) -> Result<PinRole, String> + Send + Sync>;

/// 节点定义（静态描述）
/// 
/// 定义节点的类型、Pin、处理器等，不包含运行时状态
#[derive(Clone)]
pub struct NodeDefinition {
    /// 节点类型标识符
    pub node_type: NodeDefinitionId,
    /// 显示名称
    pub display_name: String,
    /// 分类路径
    pub category: Vec<String>,
    /// UI 样式
    pub ui_style: String,
    /// 描述
    pub description: Option<String>,
    /// Pin 定义列表
    pub pins: Vec<PinDefinition>,
    /// 节点处理器
    pub processor: Option<NodeProcessor>,
    /// 是否支持动态 Pin
    pub supports_dynamic_pins: bool,
    /// 动态 Pin 配置
    pub dynamic_config: Option<DynamicPinConfig>,
}

impl NodeDefinition {
    /// 创建新的节点定义
    pub fn new(node_type: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            node_type: node_type.into(),
            display_name: display_name.into(),
            category: Vec::new(),
            ui_style: "default".into(),
            description: None,
            pins: Vec::new(),
            processor: None,
            supports_dynamic_pins: false,
            dynamic_config: None,
        }
    }
    
    /// 添加 Pin 定义
    pub fn add_pin(mut self, pin: PinDefinition) -> Self {
        self.pins.push(pin);
        self
    }
    
    /// 设置处理器
    pub fn with_processor(mut self, processor: NodeProcessor) -> Self {
        self.processor = Some(processor);
        self
    }
    
    /// 设置元数据
    pub fn with_metadata(
        mut self,
        category: Vec<String>,
        ui_style: String,
        description: Option<String>,
    ) -> Self {
        self.category = category;
        self.ui_style = ui_style;
        self.description = description;
        self
    }
    
    /// 启用动态 Pin
    pub fn with_dynamic_pins(mut self, config: DynamicPinConfig) -> Self {
        self.supports_dynamic_pins = true;
        self.dynamic_config = Some(config);
        self
    }
    
    /// 获取指定角色的 Pin 定义
    pub fn get_pin_by_role(&self, role: &PinRole) -> Option<&PinDefinition> {
        self.pins.iter().find(|p| &p.role == role)
    }
    
    /// 获取指定角色的所有 Pin 定义（用于动态组）
    pub fn get_pins_by_role(&self, role: &PinRole) -> Vec<&PinDefinition> {
        self.pins.iter().filter(|p| &p.role == role).collect()
    }
}

/// 动态 Pin 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicPinConfig {
    /// 允许的最小 Pin 数量
    pub min_count: usize,
    /// 允许的最大 Pin 数量
    pub max_count: Option<usize>,
    /// 名称模板（如 "Input {}"）
    pub name_template: String,
    /// 是否可以重新排序
    pub can_reorder: bool,
}

// ==================== 节点实例（运行时） ====================

/// 节点实例（运行时）
/// 
/// 仅包含 ID 和对定义的引用，不持有任何 Pin 或状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInstance {
    /// 节点实例 ID
    pub id: NodeId,
    /// 节点定义类型
    pub definition_type: NodeDefinitionId,
    /// 显示标题（可自定义）
    pub title: String,
    /// 动态添加的 Pin ID 列表
    pub dynamic_pins: Vec<PinId>,
    /// 变量关联（如果是变量节点）
    pub variable_id: Option<String>,
}

impl NodeInstance {
    /// 创建新的节点实例
    pub fn new(definition_type: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            definition_type: definition_type.into(),
            title: title.into(),
            dynamic_pins: Vec::new(),
            variable_id: None,
        }
    }
}

// ==================== Pin 实例（运行时） ====================

/// Pin 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinState {
    /// 未初始化
    Uninitialized,
    /// 就绪
    Ready,
    /// 计算中
    Computing,
    /// 错误
    Error,
    /// 已完成
    Completed,
}

/// Pin 实例（运行时）
/// 
/// 包含 Pin 的运行时状态和值，不包含连接信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinInstance {
    /// Pin ID
    pub id: PinId,
    /// 所属节点 ID
    pub node_id: NodeId,
    /// Pin 角色
    pub role: PinRole,
    /// Pin 方向
    pub direction: PinDirection,
    /// Pin 类型
    pub kind: PinKind,
    /// 数据类型描述（仅 Data Pin）
    pub type_desc: Option<PinTypeDesc>,
    /// 显示名称
    pub display_name: String,
    /// 运行时状态
    pub state: PinState,
    /// 当前值（仅 Data Pin）
    pub value: Option<DataValue>,
    /// 用户设置的值（仅 Data Pin）
    pub user_value: Option<DataValue>,
    /// 默认值（仅 Data Pin）
    pub default_value: Option<DataValue>,
    /// 是否是动态 Pin
    pub is_dynamic: bool,
    /// 动态组 ID
    pub group_id: Option<String>,
}

impl PinInstance {
    /// 从定义创建 Pin 实例
    pub fn from_definition(node_id: NodeId, def: &PinDefinition) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            role: def.role.clone(),
            direction: def.direction,
            kind: def.kind,
            type_desc: def.type_desc.clone(),
            display_name: def.display_name.clone(),
            state: PinState::Uninitialized,
            value: None,
            user_value: None,
            default_value: None,
            is_dynamic: def.is_dynamic,
            group_id: def.group_id.clone(),
        }
    }
    
    /// 创建动态 Pin 实例
    pub fn dynamic(
        node_id: NodeId,
        role: PinRole,
        direction: PinDirection,
        kind: PinKind,
        type_desc: Option<PinTypeDesc>,
        display_name: impl Into<String>,
        group_id: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            role,
            direction,
            kind,
            type_desc,
            display_name: display_name.into(),
            state: PinState::Uninitialized,
            value: None,
            user_value: None,
            default_value: None,
            is_dynamic: true,
            group_id: Some(group_id.into()),
        }
    }
}

// ==================== Graph（运行时世界） ====================

/// 连接信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub from_pin: PinId,
    pub to_pin: PinId,
}

/// Graph - 运行时真实世界
/// 
/// 管理所有节点实例、Pin 实例、连接关系和状态
pub struct Graph {
    /// 节点实例映射
    pub nodes: HashMap<NodeId, NodeInstance>,
    /// Pin 实例映射
    pub pins: HashMap<PinId, PinInstance>,
    /// 连接关系（from_pin -> to_pins）
    pub connections: HashMap<PinId, Vec<PinId>>,
    /// Pin 到节点的反向映射
    pub pin_to_node: HashMap<PinId, NodeId>,
    /// 节点到 Pin 的映射
    pub node_to_pins: HashMap<NodeId, Vec<PinId>>,
    /// 节点状态
    pub node_states: HashMap<NodeId, NodeState>,
}

impl Graph {
    /// 创建新的 Graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            pins: HashMap::new(),
            connections: HashMap::new(),
            pin_to_node: HashMap::new(),
            node_to_pins: HashMap::new(),
            node_states: HashMap::new(),
        }
    }
    
    /// 添加节点实例
    pub fn add_node(&mut self, node: NodeInstance, definition: &NodeDefinition) -> NodeId {
        let node_id = node.id;
        
        // 创建节点的所有 Pin 实例
        let mut pin_ids = Vec::new();
        for pin_def in &definition.pins {
            let pin = PinInstance::from_definition(node_id, pin_def);
            let pin_id = pin.id;
            
            self.pins.insert(pin_id, pin);
            self.pin_to_node.insert(pin_id, node_id);
            pin_ids.push(pin_id);
        }
        
        self.node_to_pins.insert(node_id, pin_ids);
        self.node_states.insert(node_id, NodeState::Idle);
        self.nodes.insert(node_id, node);
        
        node_id
    }
    
    /// 移除节点
    pub fn remove_node(&mut self, node_id: NodeId) {
        // 移除节点的所有 Pin
        if let Some(pin_ids) = self.node_to_pins.remove(&node_id) {
            for pin_id in pin_ids {
                self.pins.remove(&pin_id);
                self.pin_to_node.remove(&pin_id);
                // 移除相关连接
                self.connections.remove(&pin_id);
                // 移除指向此 Pin 的连接
                for targets in self.connections.values_mut() {
                    targets.retain(|&id| id != pin_id);
                }
            }
        }
        
        self.nodes.remove(&node_id);
        self.node_states.remove(&node_id);
    }
    
    /// 连接两个 Pin
    pub fn connect(&mut self, from_pin: PinId, to_pin: PinId) -> Result<(), String> {
        // 验证 Pin 存在
        if !self.pins.contains_key(&from_pin) {
            return Err(format!("Source pin not found: {:?}", from_pin));
        }
        if !self.pins.contains_key(&to_pin) {
            return Err(format!("Target pin not found: {:?}", to_pin));
        }
        
        // 验证方向
        let from = self.pins.get(&from_pin).unwrap();
        let to = self.pins.get(&to_pin).unwrap();
        
        if from.direction != PinDirection::Output {
            return Err("Source pin must be an output".to_string());
        }
        if to.direction != PinDirection::Input {
            return Err("Target pin must be an input".to_string());
        }
        
        // 验证类型匹配
        if from.kind != to.kind {
            return Err(format!("Pin kind mismatch: {:?} vs {:?}", from.kind, to.kind));
        }
        
        // 建立连接
        self.connections
            .entry(from_pin)
            .or_insert_with(Vec::new)
            .push(to_pin);
        
        Ok(())
    }
    
    /// 断开连接
    pub fn disconnect(&mut self, from_pin: PinId, to_pin: PinId) {
        if let Some(targets) = self.connections.get_mut(&from_pin) {
            targets.retain(|&id| id != to_pin);
        }
    }
    
    /// 获取 Pin 的下游连接
    pub fn get_downstream(&self, pin_id: PinId) -> Vec<PinId> {
        self.connections.get(&pin_id).cloned().unwrap_or_default()
    }
    
    /// 获取 Pin 的上游连接
    pub fn get_upstream(&self, pin_id: PinId) -> Option<PinId> {
        for (from, targets) in &self.connections {
            if targets.contains(&pin_id) {
                return Some(*from);
            }
        }
        None
    }
    
    /// 获取节点的所有 Pin
    pub fn get_node_pins(&self, node_id: NodeId) -> Vec<&PinInstance> {
        self.node_to_pins
            .get(&node_id)
            .map(|pin_ids| {
                pin_ids.iter()
                    .filter_map(|id| self.pins.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// 通过角色获取节点的 Pin
    pub fn get_pin_by_role(&self, node_id: NodeId, role: &PinRole) -> Option<&PinInstance> {
        self.get_node_pins(node_id)
            .into_iter()
            .find(|p| &p.role == role)
    }
    
    /// 通过角色获取节点的所有 Pin（用于动态组）
    pub fn get_pins_by_role(&self, node_id: NodeId, role: &PinRole) -> Vec<&PinInstance> {
        self.get_node_pins(node_id)
            .into_iter()
            .filter(|p| &p.role == role)
            .collect()
    }
    
    /// 获取 Pin 的值
    pub fn get_pin_value(&self, pin_id: PinId) -> Option<&DataValue> {
        self.pins.get(&pin_id)?.value.as_ref()
    }
    
    /// 设置 Pin 的值
    pub fn set_pin_value(&mut self, pin_id: PinId, value: DataValue) -> Result<(), String> {
        let pin = self.pins.get_mut(&pin_id)
            .ok_or_else(|| format!("Pin not found: {:?}", pin_id))?;
        
        if pin.kind != PinKind::Data {
            return Err("Cannot set value on exec pin".to_string());
        }
        
        pin.value = Some(value);
        pin.state = PinState::Ready;
        Ok(())
    }
    
    /// 获取节点状态
    pub fn get_node_state(&self, node_id: NodeId) -> Option<NodeState> {
        self.node_states.get(&node_id).copied()
    }
    
    /// 设置节点状态
    pub fn set_node_state(&mut self, node_id: NodeId, state: NodeState) {
        self.node_states.insert(node_id, state);
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== 节点执行上下文 ====================

/// 节点执行上下文 Trait
/// 
/// 提供基于语义角色的 Pin 访问 API，隐藏底层实现细节
pub trait NodeExecutionContext {
    /// 通过角色获取单个输入值
    fn get_input_by_role(&self, role: &PinRole) -> Result<DataValue, String>;
    
    /// 通过角色获取多个输入值（用于动态组）
    fn get_inputs_by_role(&self, role: &PinRole) -> Result<Vec<DataValue>, String>;
    
    /// 通过角色设置单个输出值
    fn emit_output_by_role(&mut self, role: &PinRole, value: DataValue) -> Result<(), String>;
    
    /// 通过角色设置多个输出值（用于动态组）
    fn emit_outputs_by_role(&mut self, role: &PinRole, values: Vec<DataValue>) -> Result<(), String>;
    
    /// 检查输入是否已连接
    fn is_input_connected(&self, role: &PinRole) -> bool;
    
    /// 获取当前节点 ID
    fn node_id(&self) -> NodeId;
    
    /// 记录日志
    fn log(&mut self, message: String);
    
    /// 记录错误
    fn error(&mut self, message: String);
}

/// 具体的执行上下文实现
pub struct GraphExecutionContext<'a> {
    pub graph: &'a mut Graph,
    pub node_id: NodeId,
    pub logs: Vec<String>,
}

impl<'a> GraphExecutionContext<'a> {
    pub fn new(graph: &'a mut Graph, node_id: NodeId) -> Self {
        Self {
            graph,
            node_id,
            logs: Vec::new(),
        }
    }
}

impl<'a> NodeExecutionContext for GraphExecutionContext<'a> {
    fn get_input_by_role(&self, role: &PinRole) -> Result<DataValue, String> {
        let pin = self.graph.get_pin_by_role(self.node_id, role)
            .ok_or_else(|| format!("Input pin with role {:?} not found", role))?;
        
        if pin.direction != PinDirection::Input {
            return Err(format!("Pin {:?} is not an input", role));
        }
        
        // 检查是否有上游连接
        if let Some(upstream_pin) = self.graph.get_upstream(pin.id) {
            // 从上游获取值
            self.graph.get_pin_value(upstream_pin)
                .cloned()
                .ok_or_else(|| format!("Upstream pin has no value"))
        } else {
            // 使用用户值或默认值
            pin.user_value.clone()
                .or_else(|| pin.default_value.clone())
                .ok_or_else(|| format!("Pin {:?} has no value", role))
        }
    }
    
    fn get_inputs_by_role(&self, role: &PinRole) -> Result<Vec<DataValue>, String> {
        let pins = self.graph.get_pins_by_role(self.node_id, role);
        
        if pins.is_empty() {
            return Err(format!("No input pins with role {:?} found", role));
        }
        
        let mut values = Vec::new();
        for pin in pins {
            if pin.direction != PinDirection::Input {
                continue;
            }
            
            // 检查是否有上游连接
            if let Some(upstream_pin) = self.graph.get_upstream(pin.id) {
                if let Some(value) = self.graph.get_pin_value(upstream_pin) {
                    values.push(value.clone());
                }
            } else if let Some(value) = pin.user_value.as_ref().or(pin.default_value.as_ref()) {
                values.push(value.clone());
            }
        }
        
        Ok(values)
    }
    
    fn emit_output_by_role(&mut self, role: &PinRole, value: DataValue) -> Result<(), String> {
        let pin = self.graph.get_pin_by_role(self.node_id, role)
            .ok_or_else(|| format!("Output pin with role {:?} not found", role))?;
        
        if pin.direction != PinDirection::Output {
            return Err(format!("Pin {:?} is not an output", role));
        }
        
        let pin_id = pin.id;
        self.graph.set_pin_value(pin_id, value)
    }
    
    fn emit_outputs_by_role(&mut self, role: &PinRole, values: Vec<DataValue>) -> Result<(), String> {
        let pins = self.graph.get_pins_by_role(self.node_id, role);
        
        if pins.is_empty() {
            return Err(format!("No output pins with role {:?} found", role));
        }
        
        let pin_ids: Vec<PinId> = pins.iter()
            .filter(|p| p.direction == PinDirection::Output)
            .map(|p| p.id)
            .collect();
        
        if pin_ids.len() != values.len() {
            return Err(format!(
                "Value count mismatch: {} pins, {} values",
                pin_ids.len(),
                values.len()
            ));
        }
        
        for (pin_id, value) in pin_ids.into_iter().zip(values) {
            self.graph.set_pin_value(pin_id, value)?;
        }
        
        Ok(())
    }
    
    fn is_input_connected(&self, role: &PinRole) -> bool {
        if let Some(pin) = self.graph.get_pin_by_role(self.node_id, role) {
            self.graph.get_upstream(pin.id).is_some()
        } else {
            false
        }
    }
    
    fn node_id(&self) -> NodeId {
        self.node_id
    }
    
    fn log(&mut self, message: String) {
        self.logs.push(message);
    }
    
    fn error(&mut self, message: String) {
        self.logs.push(format!("ERROR: {}", message));
    }
}
