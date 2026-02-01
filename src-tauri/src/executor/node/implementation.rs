//! 节点实现模块
//!
//! 实现 GenericNode：通用的节点容器

use dashmap::DashMap;
use serde::{ser::SerializeStruct, Serialize, Serializer, Deserialize};
use serde_json::Value;
use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;

use super::traits::Node;
use super::types::{NodeId, NodeState};
use crate::executor::error::{ExecutionResult, NodeResult};
use crate::executor::node::NodeData;
use crate::executor::pin::{
    BasePin, DataPin, ExecPin, GenericInDataPin, GenericInExecPin, GenericOutDataPin,
    GenericOutExecPin, InDataPin, OutDataPin, PinId,
};
use crate::executor::processors::ExecutionContextTrait;
use crate::executor::value::PinTypeDesc;

// ==================== 动态 Pin 支持数据结构 ====================

/// 动态 Pin 类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DynamicPinType {
    Exec,
    Data,
}

/// Pin 方向
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PinDirection {
    Input,
    Output,
}

/// 动态 Pin 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicPinConfig {
    pub pin_type: DynamicPinType,
    pub direction: PinDirection,
    pub name_template: String,        // "Then {}", "Case {}", "Input {}"
    pub data_type: PinTypeDesc,
    pub min_count: usize,
    pub max_count: Option<usize>,
    pub can_reorder: bool,
}

/// Pin 变更事件
#[derive(Debug, Clone)]
pub enum PinChangeEvent {
    PinAdded { pin_id: PinId, pin_type: String },
    PinRemoved { pin_id: PinId, pin_type: String },
    PinReordered { old_order: Vec<PinId>, new_order: Vec<PinId> },
}

/// 动态 Pin 信息（用于序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicPinInfo {
    pub pin_id: String,
    pub pin_type: String,
    pub direction: String,
    pub name: String,
    pub data_type: String,
    pub is_dynamic: bool,
}

/// 处理器生成器类型
pub type ProcessorGenerator = Box<dyn Fn(&GenericNode) -> FlowProcessor + Send + Sync>;
pub type FlowProcessor = Box<dyn Fn(&mut dyn ExecutionContextTrait, &NodeData) -> Result<String, String> + Send + Sync + 'static>;
pub type PinChangeCallback = Box<dyn Fn(&GenericNode, PinChangeEvent) -> Result<(), String> + Send + Sync>;

/// 节点动态能力描述
pub struct NodeDynamicCapability {
    pub can_add_pins: bool,
    pub dynamic_configs: Vec<DynamicPinConfig>,
    pub processor_generator: Option<ProcessorGenerator>,
}

/// 泛型节点实现
///
/// 使用 DashMap 存储 Pin，支持并发访问
pub struct GenericNode {
    id: NodeId,
    title: RwLock<String>,
    node_type: String,
    state: RwLock<NodeState>,

    // 元数据
    category: Vec<String>,
    ui_style: String,
    description: Option<String>,

    // 变量关联
    variable_id: RwLock<Option<String>>,

    // 使用 DashMap 支持并发访问（使用 PinId 作为键）
    in_data_pins: DashMap<PinId, Arc<GenericInDataPin>>,
    out_data_pins: DashMap<PinId, Arc<GenericOutDataPin>>,
    in_exec_pins: DashMap<PinId, Arc<GenericInExecPin>>,
    out_exec_pins: DashMap<PinId, Arc<GenericOutExecPin>>,
    
    // Pin 顺序追踪器
    input_order: RwLock<Vec<PinId>>,
    output_order: RwLock<Vec<PinId>>,
    
    // 处理器
    flow_processor: Mutex<
        Option<
            Box<
                dyn Fn(&mut dyn ExecutionContextTrait, &NodeData) -> Result<String, String>
                    + Send
                    + Sync
                    + 'static,
            >,
        >,
    >,
    data_processor: Mutex<
        Option<
            Box<
                dyn Fn(&mut dyn ExecutionContextTrait, &NodeData, &str) -> Value
                    + Send
                    + Sync
                    + 'static,
            >,
        >,
    >,
    
    // ==================== 动态 Pin 支持字段 ====================
    
    /// 动态能力配置
    dynamic_capability: RwLock<Option<NodeDynamicCapability>>,
    
    /// 动态 Pin 计数器（用于生成唯一名称）
    dynamic_pin_counter: RwLock<HashMap<String, usize>>,
    
    /// Pin 变更回调
    pin_change_callbacks: RwLock<Vec<PinChangeCallback>>,
    
    /// 动态添加的 Pin 跟踪（用于区分静态和动态 Pin）
    dynamic_pins: RwLock<HashMap<PinId, DynamicPinInfo>>,
}

impl GenericNode {
    pub fn new_prototype(node_type: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::nil(),
            title: RwLock::new(title.into()),
            node_type: node_type.into(),
            state: RwLock::new(NodeState::Idle),
            category: Vec::new(),
            ui_style: "default".into(),
            description: None,
            variable_id: RwLock::new(None),
            in_data_pins: DashMap::new(),
            out_data_pins: DashMap::new(),
            in_exec_pins: DashMap::new(),
            out_exec_pins: DashMap::new(),
            input_order: RwLock::new(Vec::new()),
            output_order: RwLock::new(Vec::new()),
            flow_processor: Mutex::new(None),
            data_processor: Mutex::new(None),
            // 动态 Pin 支持字段初始化
            dynamic_capability: RwLock::new(None),
            dynamic_pin_counter: RwLock::new(HashMap::new()),
            pin_change_callbacks: RwLock::new(Vec::new()),
            dynamic_pins: RwLock::new(HashMap::new()),
        }
    }

    pub fn new(id: NodeId, title: impl Into<String>, node_type: impl Into<String>) -> Self {
        Self {
            id,
            title: RwLock::new(title.into()),
            node_type: node_type.into(),
            state: RwLock::new(NodeState::Idle),
            category: Vec::new(),
            ui_style: "default".into(),
            description: None,
            variable_id: RwLock::new(None),
            in_data_pins: DashMap::new(),
            out_data_pins: DashMap::new(),
            in_exec_pins: DashMap::new(),
            out_exec_pins: DashMap::new(),
            input_order: RwLock::new(Vec::new()),
            output_order: RwLock::new(Vec::new()),
            flow_processor: Mutex::new(None),
            data_processor: Mutex::new(None),
            // 动态 Pin 支持字段初始化
            dynamic_capability: RwLock::new(None),
            dynamic_pin_counter: RwLock::new(HashMap::new()),
            pin_change_callbacks: RwLock::new(Vec::new()),
            dynamic_pins: RwLock::new(HashMap::new()),
        }
    }

    pub fn set_variable_id(&self, id: Option<String>) {
        *self.variable_id.write().unwrap() = id;
    }

    pub fn variable_id(&self) -> Option<String> {
        self.variable_id.read().unwrap().clone()
    }

    pub fn set_metadata(
        &mut self,
        category: Vec<String>,
        ui_style: String,
        description: Option<String>,
    ) {
        self.category = category;
        self.ui_style = ui_style;
        self.description = description;
    }

    pub fn add_input(&self, pin: GenericInDataPin) -> Arc<GenericInDataPin> {
        let id = pin.id();
        let pin = Arc::new(pin);
        self.in_data_pins.insert(id, pin.clone());
        // 添加到输入顺序追踪器
        self.input_order.write().unwrap().push(id);
        pin
    }

    pub fn add_output(&self, pin: GenericOutDataPin) -> Arc<GenericOutDataPin> {
        let id = pin.id();
        let pin = Arc::new(pin);
        self.out_data_pins.insert(id, pin.clone());
        // 添加到输出顺序追踪器
        self.output_order.write().unwrap().push(id);
        pin
    }

    pub fn add_in_exec_pin(&self, pin: GenericInExecPin) -> Arc<GenericInExecPin> {
        let id = pin.id();
        let pin = Arc::new(pin);
        self.in_exec_pins.insert(id, pin.clone());
        // 添加到输入顺序追踪器
        self.input_order.write().unwrap().push(id);
        pin
    }

    pub fn add_out_exec_pin(&self, pin: GenericOutExecPin) -> Arc<GenericOutExecPin> {
        let id = pin.id();
        let pin = Arc::new(pin);
        self.out_exec_pins.insert(id, pin.clone());
        // 添加到输出顺序追踪器
        self.output_order.write().unwrap().push(id);
        pin
    }

    // 向后兼容:add_exec_pin 作为输出 exec pin(大多数节点使用)
    pub fn add_exec_pin(&self, pin: GenericOutExecPin) -> Arc<GenericOutExecPin> {
        self.add_out_exec_pin(pin)
    }

    pub fn set_flow_processor(
        &self,
        processor: Box<
            dyn Fn(&mut dyn ExecutionContextTrait, &NodeData) -> Result<String, String>
                + Send
                + Sync
                + 'static,
        >,
    ) {
        *self.flow_processor.lock().unwrap() = Some(processor);
    }

    pub fn set_data_processor(
        &self,
        processor: Box<
            dyn Fn(&mut dyn ExecutionContextTrait, &NodeData, &str) -> Value
                + Send
                + Sync
                + 'static,
        >,
    ) {
        *self.data_processor.lock().unwrap() = Some(processor);
    }

    pub fn process_flow(
        &self,
        ctx: &mut dyn ExecutionContextTrait,
        node: &NodeData,
    ) -> Result<String, String> {
        if let Some(p) = self.flow_processor.lock().unwrap().as_ref() {
            p(ctx, node)
        } else {
            Ok("".into())
        }
    }

    pub fn process_data(
        &self,
        ctx: &mut dyn ExecutionContextTrait,
        node: &NodeData,
        pin_id: &str,
    ) -> Value {
        if let Some(p) = self.data_processor.lock().unwrap().as_ref() {
            p(ctx, node, pin_id)
        } else {
            Value::Null
        }
    }

    pub fn node_type(&self) -> &str {
        &self.node_type
    }

    /// 获取节点的执行模型
    pub fn execution_model(&self) -> crate::executor::types::ExecutionModel {
        use crate::executor::types::ExecutionModel;
        
        let has_in_exec = !self.in_exec_pins.is_empty();
        let has_out_exec = !self.out_exec_pins.is_empty();
        let has_data = !self.in_data_pins.is_empty() || !self.out_data_pins.is_empty();
        
        match (has_in_exec, has_out_exec, has_data) {
            // 只有输出 exec，没有输入 exec = Event 节点
            (false, true, false) => ExecutionModel::Event,
            
            // 有输出 exec 和 data = Hybrid 节点（Event 也可能有数据）
            (false, true, true) => ExecutionModel::Hybrid,
            
            // 有输入 exec 和 data = Hybrid 节点
            (true, _, true) => ExecutionModel::Hybrid,
            
            // 只有 exec pin（有输入） = ControlFlow 节点
            (true, _, false) => ExecutionModel::ControlFlow,
            
            // 只有 data pin = DataFlow 节点
            (false, false, true) => ExecutionModel::DataFlow,
            
            // 什么都没有 = 无效节点，默认为 DataFlow
            (false, false, false) => ExecutionModel::DataFlow,
        }
    }

    pub fn input_names(&self) -> Vec<String> {
        self.in_data_pins
            .iter()
            .map(|e| e.value().name().to_string())
            .collect()
    }

    pub fn output_names(&self) -> Vec<String> {
        self.out_data_pins
            .iter()
            .map(|e| e.value().name().to_string())
            .collect()
    }

    pub fn get_input_by_name(&self, name: &str) -> Option<Arc<GenericInDataPin>> {
        self.in_data_pins
            .iter()
            .find(|e| e.value().name() == name)
            .map(|e| e.value().clone())
    }

    pub fn get_output_by_name(&self, name: &str) -> Option<Arc<GenericOutDataPin>> {
        self.out_data_pins
            .iter()
            .find(|e| e.value().name() == name)
            .map(|e| e.value().clone())
    }

    pub fn get_input_concrete(&self, id: &PinId) -> Option<Arc<GenericInDataPin>> {
        self.in_data_pins.get(id).map(|p| p.clone())
    }

    pub fn get_output_concrete(&self, id: &PinId) -> Option<Arc<GenericOutDataPin>> {
        self.out_data_pins.get(id).map(|p| p.clone())
    }

    pub fn get_in_exec_pin(&self, id: &PinId) -> Option<Arc<GenericInExecPin>> {
        self.in_exec_pins.get(id).map(|p| p.clone())
    }

    pub fn get_out_exec_pin(&self, id: &PinId) -> Option<Arc<GenericOutExecPin>> {
        self.out_exec_pins.get(id).map(|p| p.clone())
    }

    pub fn get_in_exec_pin_by_name(&self, name: &str) -> Option<Arc<GenericInExecPin>> {
        self.in_exec_pins
            .iter()
            .find(|e| e.value().name() == name)
            .map(|e| e.value().clone())
    }

    pub fn get_out_exec_pin_by_name(&self, name: &str) -> Option<Arc<GenericOutExecPin>> {
        self.out_exec_pins
            .iter()
            .find(|e| e.value().name() == name)
            .map(|e| e.value().clone())
    }

    /// 获取输入 Pin 的顺序
    pub fn get_input_order(&self) -> Vec<PinId> {
        self.input_order.read().unwrap().clone()
    }

    /// 获取输出 Pin 的顺序
    pub fn get_output_order(&self) -> Vec<PinId> {
        self.output_order.read().unwrap().clone()
    }

    /// 重新排序输入 Pin
    pub fn reorder_inputs(&self, new_order: Vec<PinId>) -> Result<(), String> {
        let mut input_order = self.input_order.write().unwrap();
        
        // 验证新顺序包含所有现有的输入 Pin
        if new_order.len() != input_order.len() {
            return Err("New order length doesn't match current input count".to_string());
        }
        
        for &pin_id in &new_order {
            if !input_order.contains(&pin_id) {
                return Err(format!("Pin ID {:?} not found in current inputs", pin_id));
            }
        }
        
        *input_order = new_order;
        Ok(())
    }

    /// 重新排序输出 Pin
    pub fn reorder_outputs(&self, new_order: Vec<PinId>) -> Result<(), String> {
        let mut output_order = self.output_order.write().unwrap();
        
        // 验证新顺序包含所有现有的输出 Pin
        if new_order.len() != output_order.len() {
            return Err("New order length doesn't match current output count".to_string());
        }
        
        for &pin_id in &new_order {
            if !output_order.contains(&pin_id) {
                return Err(format!("Pin ID {:?} not found in current outputs", pin_id));
            }
        }
        
        *output_order = new_order;
        Ok(())
    }

    /// 移除输入 Pin 并更新顺序
    pub fn remove_input(&self, pin_id: PinId) -> bool {
        let removed_data = self.in_data_pins.remove(&pin_id).is_some();
        let removed_exec = self.in_exec_pins.remove(&pin_id).is_some();
        
        if removed_data || removed_exec {
            let mut input_order = self.input_order.write().unwrap();
            input_order.retain(|&id| id != pin_id);
            true
        } else {
            false
        }
    }

    /// 移除输出 Pin 并更新顺序
    pub fn remove_output(&self, pin_id: PinId) -> bool {
        let removed_data = self.out_data_pins.remove(&pin_id).is_some();
        let removed_exec = self.out_exec_pins.remove(&pin_id).is_some();
        
        if removed_data || removed_exec {
            let mut output_order = self.output_order.write().unwrap();
            output_order.retain(|&id| id != pin_id);
            true
        } else {
            false
        }
    }

    /// 获取按顺序排列的所有输入 Pin 信息（用于调试）
    pub fn get_ordered_input_info(&self) -> Vec<(PinId, String, String)> {
        let input_order = self.input_order.read().unwrap();
        let mut result = Vec::new();
        
        for &pin_id in input_order.iter() {
            if let Some(exec_pin) = self.in_exec_pins.get(&pin_id) {
                result.push((pin_id, exec_pin.value().name().to_string(), "exec".to_string()));
            } else if let Some(data_pin) = self.in_data_pins.get(&pin_id) {
                result.push((pin_id, data_pin.value().name().to_string(), data_pin.value().data_type().to_string()));
            }
        }
        
        result
    }

    /// 获取按顺序排列的所有输出 Pin 信息（用于调试）
    pub fn get_ordered_output_info(&self) -> Vec<(PinId, String, String)> {
        let output_order = self.output_order.read().unwrap();
        let mut result = Vec::new();
        
        for &pin_id in output_order.iter() {
            if let Some(exec_pin) = self.out_exec_pins.get(&pin_id) {
                result.push((pin_id, exec_pin.value().name().to_string(), "exec".to_string()));
            } else if let Some(data_pin) = self.out_data_pins.get(&pin_id) {
                result.push((pin_id, data_pin.value().name().to_string(), data_pin.value().data_type().to_string()));
            }
        }
        
        result
    }

    // ==================== 动态 Pin 支持方法 ====================

    /// 设置动态能力
    pub fn set_dynamic_capability(&self, capability: NodeDynamicCapability) {
        *self.dynamic_capability.write().unwrap() = Some(capability);
    }
    
    /// 检查是否支持动态 Pin
    pub fn supports_dynamic_pins(&self) -> bool {
        self.dynamic_capability.read().unwrap().as_ref()
            .map(|cap| cap.can_add_pins)
            .unwrap_or(false)
    }
    
    /// 获取动态 Pin 约束
    pub fn get_dynamic_constraints(&self, pin_type: &str, direction: &PinDirection) -> Option<DynamicPinConfig> {
        self.dynamic_capability.read().unwrap().as_ref()?
            .dynamic_configs.iter()
            .find(|config| {
                config.pin_type.matches(pin_type) && &config.direction == direction
            })
            .cloned()
    }
    
    /// 动态添加 Pin
    pub fn add_dynamic_pin(&self, config: &DynamicPinConfig) -> Result<PinId, String> {
        // 验证是否可以添加
        self.validate_pin_addition(config)?;
        
        // 生成 Pin 名称
        let pin_name = self.generate_pin_name(config)?;
        let pin_id = uuid::Uuid::new_v4();
        
        // 根据类型添加 Pin
        match (&config.pin_type, &config.direction) {
            (DynamicPinType::Exec, PinDirection::Output) => {
                let pin = GenericOutExecPin::new(pin_id, &pin_name);
                self.add_out_exec_pin(pin);
            }
            (DynamicPinType::Exec, PinDirection::Input) => {
                let pin = GenericInExecPin::new(pin_id, &pin_name);
                self.add_in_exec_pin(pin);
            }
            (DynamicPinType::Data, PinDirection::Output) => {
                let pin = GenericOutDataPin::new(pin_id, &pin_name, config.data_type.clone());
                self.add_output(pin);
            }
            (DynamicPinType::Data, PinDirection::Input) => {
                let pin = GenericInDataPin::new(pin_id, &pin_name, config.data_type.clone());
                self.add_input(pin);
            }
        }
        
        // 记录为动态 Pin
        let pin_info = DynamicPinInfo {
            pin_id: pin_id.to_string(),
            pin_type: format!("{:?}", config.pin_type),
            direction: format!("{:?}", config.direction),
            name: pin_name.clone(),
            data_type: config.data_type.type_string(),
            is_dynamic: true,
        };
        self.dynamic_pins.write().unwrap().insert(pin_id, pin_info);
        
        // 触发回调
        self.notify_pin_change(PinChangeEvent::PinAdded {
            pin_id,
            pin_type: format!("{:?}_{:?}", config.pin_type, config.direction),
        })?;
        
        // 重新生成处理器
        self.regenerate_processor()?;
        
        Ok(pin_id)
    }
    
    /// 动态移除 Pin
    pub fn remove_dynamic_pin(&self, pin_id: PinId) -> Result<(), String> {
        // 检查是否是动态 Pin
        if !self.dynamic_pins.read().unwrap().contains_key(&pin_id) {
            return Err("Cannot remove static pin".to_string());
        }
        
        // 验证是否可以移除
        self.validate_pin_removal(pin_id)?;
        
        // 确定 Pin 类型
        let pin_type = self.get_pin_type(pin_id)?;
        
        // 移除 Pin
        let removed = match pin_type.as_str() {
            "exec_input" => self.remove_input(pin_id),
            "exec_output" => self.remove_output(pin_id),
            "data_input" => self.remove_input(pin_id),
            "data_output" => self.remove_output(pin_id),
            _ => return Err(format!("Unknown pin type: {}", pin_type)),
        };
        
        if !removed {
            return Err(format!("Failed to remove pin: {:?}", pin_id));
        }
        
        // 从动态 Pin 记录中移除
        self.dynamic_pins.write().unwrap().remove(&pin_id);
        
        // 触发回调
        self.notify_pin_change(PinChangeEvent::PinRemoved { pin_id, pin_type })?;
        
        // 重新生成处理器
        self.regenerate_processor()?;
        
        Ok(())
    }
    
    /// 重新生成处理器
    pub fn regenerate_processor(&self) -> Result<(), String> {
        if let Some(capability) = self.dynamic_capability.read().unwrap().as_ref() {
            if let Some(generator) = &capability.processor_generator {
                let new_processor = generator(self);
                *self.flow_processor.lock().unwrap() = Some(new_processor);
            }
        }
        Ok(())
    }
    
    /// 验证 Pin 添加
    fn validate_pin_addition(&self, config: &DynamicPinConfig) -> Result<(), String> {
        let current_count = self.count_pins_of_type(&config.pin_type, &config.direction);
        
        if let Some(max_count) = config.max_count {
            if current_count >= max_count {
                return Err(format!(
                    "Cannot add more pins: current={}, max={}",
                    current_count, max_count
                ));
            }
        }
        
        Ok(())
    }
    
    /// 验证 Pin 移除
    fn validate_pin_removal(&self, pin_id: PinId) -> Result<(), String> {
        // 获取 Pin 信息
        let pin_info = self.dynamic_pins.read().unwrap()
            .get(&pin_id)
            .cloned()
            .ok_or("Pin not found in dynamic pins")?;
        
        // 检查最小数量限制
        let pin_type = match pin_info.pin_type.as_str() {
            "Exec" => DynamicPinType::Exec,
            "Data" => DynamicPinType::Data,
            _ => return Err("Invalid pin type".to_string()),
        };
        
        let direction = match pin_info.direction.as_str() {
            "Input" => PinDirection::Input,
            "Output" => PinDirection::Output,
            _ => return Err("Invalid pin direction".to_string()),
        };
        
        if let Some(config) = self.get_dynamic_constraints(&pin_info.pin_type, &direction) {
            let current_count = self.count_pins_of_type(&pin_type, &direction);
            if current_count <= config.min_count {
                return Err(format!(
                    "Cannot remove pin: current={}, min={}",
                    current_count, config.min_count
                ));
            }
        }
        
        Ok(())
    }
    
    /// 生成 Pin 名称
    fn generate_pin_name(&self, config: &DynamicPinConfig) -> Result<String, String> {
        let mut counter = self.dynamic_pin_counter.write().unwrap();
        let key = format!("{:?}_{:?}", config.pin_type, config.direction);
        let count = counter.entry(key).or_insert(0);
        *count += 1;
        
        Ok(config.name_template.replace("{}", &count.to_string()))
    }
    
    /// 计算特定类型的 Pin 数量
    fn count_pins_of_type(&self, pin_type: &DynamicPinType, direction: &PinDirection) -> usize {
        match (pin_type, direction) {
            (DynamicPinType::Exec, PinDirection::Input) => self.in_exec_pins.len(),
            (DynamicPinType::Exec, PinDirection::Output) => self.out_exec_pins.len(),
            (DynamicPinType::Data, PinDirection::Input) => self.in_data_pins.len(),
            (DynamicPinType::Data, PinDirection::Output) => self.out_data_pins.len(),
        }
    }
    
    /// 获取 Pin 类型
    fn get_pin_type(&self, pin_id: PinId) -> Result<String, String> {
        if self.in_exec_pins.contains_key(&pin_id) {
            Ok("exec_input".to_string())
        } else if self.out_exec_pins.contains_key(&pin_id) {
            Ok("exec_output".to_string())
        } else if self.in_data_pins.contains_key(&pin_id) {
            Ok("data_input".to_string())
        } else if self.out_data_pins.contains_key(&pin_id) {
            Ok("data_output".to_string())
        } else {
            Err(format!("Pin not found: {:?}", pin_id))
        }
    }
    
    /// 通知 Pin 变更
    fn notify_pin_change(&self, event: PinChangeEvent) -> Result<(), String> {
        let callbacks = self.pin_change_callbacks.read().unwrap();
        for callback in callbacks.iter() {
            callback(self, event.clone())?;
        }
        Ok(())
    }
    
    /// 添加 Pin 变更回调
    pub fn add_pin_change_callback(&self, callback: PinChangeCallback) {
        self.pin_change_callbacks.write().unwrap().push(callback);
    }
    
    /// 获取动态 Pin 信息（用于序列化）
    pub fn get_dynamic_pin_info(&self) -> Vec<DynamicPinInfo> {
        self.dynamic_pins.read().unwrap().values().cloned().collect()
    }
    
    /// 从动态 Pin 信息重建（用于反序列化）
    pub fn rebuild_from_dynamic_info(&self, pin_infos: Vec<DynamicPinInfo>) -> Result<(), String> {
        for pin_info in pin_infos {
            let pin_type = match pin_info.pin_type.as_str() {
                "Exec" => DynamicPinType::Exec,
                "Data" => DynamicPinType::Data,
                _ => continue,
            };
            
            let direction = match pin_info.direction.as_str() {
                "Input" => PinDirection::Input,
                "Output" => PinDirection::Output,
                _ => continue,
            };
            
            let config = DynamicPinConfig {
                pin_type,
                direction,
                name_template: pin_info.name.clone(),
                data_type: PinTypeDesc::from_string(&pin_info.data_type),
                min_count: 0,
                max_count: None,
                can_reorder: true,
            };
            
            // 直接添加 Pin，不通过动态添加流程（避免重复验证）
            let pin_id = uuid::Uuid::parse_str(&pin_info.pin_id)
                .map_err(|e| format!("Invalid pin ID: {}", e))?;
            
            match (&config.pin_type, &config.direction) {
                (DynamicPinType::Exec, PinDirection::Output) => {
                    let pin = GenericOutExecPin::new(pin_id, &pin_info.name);
                    self.add_out_exec_pin(pin);
                }
                (DynamicPinType::Exec, PinDirection::Input) => {
                    let pin = GenericInExecPin::new(pin_id, &pin_info.name);
                    self.add_in_exec_pin(pin);
                }
                (DynamicPinType::Data, PinDirection::Output) => {
                    let pin = GenericOutDataPin::new(pin_id, &pin_info.name, config.data_type.clone());
                    self.add_output(pin);
                }
                (DynamicPinType::Data, PinDirection::Input) => {
                    let pin = GenericInDataPin::new(pin_id, &pin_info.name, config.data_type.clone());
                    self.add_input(pin);
                }
            }
            
            // 记录为动态 Pin
            self.dynamic_pins.write().unwrap().insert(pin_id, pin_info);
        }
        
        // 重新生成处理器
        self.regenerate_processor()?;
        
        Ok(())
    }

    /// 获取所有动态输出执行 Pin 的名称（用于处理器生成）
    pub fn get_dynamic_exec_output_names(&self) -> Vec<String> {
        let output_order = self.output_order.read().unwrap();
        let mut names = Vec::new();
        
        for &pin_id in output_order.iter() {
            if let Some(exec_pin) = self.out_exec_pins.get(&pin_id) {
                names.push(exec_pin.value().name().to_string());
            }
        }
        
        names
    }
}

impl Serialize for GenericNode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("NodeDefinition", 7)?;
        state.serialize_field("node_type", &self.node_type)?;
        state.serialize_field("category", &self.category)?;
        state.serialize_field("title", &*self.title.read().unwrap())?;
        state.serialize_field("ui_style", &self.ui_style)?;
        state.serialize_field("description", &self.description)?;

        // 构造 Pin 定义列表 (满足前端 NodeDefinition 格式)
        #[derive(Serialize)]
        struct PinDefProxy {
            name: String,
            #[serde(rename = "type")]
            pin_type: String,
            #[serde(rename = "defaultValue")]
            default_value: Option<Value>,
            #[serde(rename = "isArray")]
            is_array: bool,
        }

        let mut inputs = Vec::new();
        let mut outputs = Vec::new();

        // 按照输入顺序追踪器的顺序收集 Pin
        let input_order = self.input_order.read().unwrap();
        for &pin_id in input_order.iter() {
            // 检查是否是执行 Pin
            if let Some(exec_pin) = self.in_exec_pins.get(&pin_id) {
                inputs.push(PinDefProxy {
                    name: exec_pin.value().name().to_string(),
                    pin_type: "exec".into(),
                    default_value: None,
                    is_array: false,
                });
            }
            // 检查是否是数据 Pin
            else if let Some(data_pin) = self.in_data_pins.get(&pin_id) {
                inputs.push(PinDefProxy {
                    name: data_pin.value().name().to_string(),
                    pin_type: data_pin.value().data_type().to_string(),
                    default_value: None,
                    is_array: false,
                });
            }
        }

        // 按照输出顺序追踪器的顺序收集 Pin
        let output_order = self.output_order.read().unwrap();
        for &pin_id in output_order.iter() {
            // 检查是否是执行 Pin
            if let Some(exec_pin) = self.out_exec_pins.get(&pin_id) {
                outputs.push(PinDefProxy {
                    name: exec_pin.value().name().to_string(),
                    pin_type: "exec".into(),
                    default_value: None,
                    is_array: false,
                });
            }
            // 检查是否是数据 Pin
            else if let Some(data_pin) = self.out_data_pins.get(&pin_id) {
                outputs.push(PinDefProxy {
                    name: data_pin.value().name().to_string(),
                    pin_type: data_pin.value().data_type().to_string(),
                    default_value: None,
                    is_array: false,
                });
            }
        }

        state.serialize_field("inputs", &inputs)?;
        state.serialize_field("outputs", &outputs)?;
        state.end()
    }
}

impl Node for GenericNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn name(&self) -> &str {
        "GenericNode"
    }
    fn set_name(&mut self, name: String) {
        *self.title.write().unwrap() = name;
    }
    fn state(&self) -> NodeState {
        *self.state.read().unwrap()
    }
    fn execute(&mut self) -> ExecutionResult<()> {
        Ok(())
    }
    fn reset(&mut self) -> NodeResult<()> {
        Ok(())
    }
    fn dispose(&mut self) -> NodeResult<()> {
        Ok(())
    }
    fn inputs(&self) -> Vec<Arc<dyn InDataPin>> {
        let input_order = self.input_order.read().unwrap();
        let mut result = Vec::new();
        
        for &pin_id in input_order.iter() {
            if let Some(data_pin) = self.in_data_pins.get(&pin_id) {
                result.push(data_pin.value().clone() as Arc<dyn InDataPin>);
            }
        }
        
        result
    }
    
    fn outputs(&self) -> Vec<Arc<dyn OutDataPin>> {
        let output_order = self.output_order.read().unwrap();
        let mut result = Vec::new();
        
        for &pin_id in output_order.iter() {
            if let Some(data_pin) = self.out_data_pins.get(&pin_id) {
                result.push(data_pin.value().clone() as Arc<dyn OutDataPin>);
            }
        }
        
        result
    }
    fn exec_pins(&self) -> Vec<Arc<dyn ExecPin>> {
        let mut pins: Vec<Arc<dyn ExecPin>> = Vec::new();
        for e in self.in_exec_pins.iter() {
            pins.push(e.value().clone() as Arc<dyn ExecPin>);
        }
        for e in self.out_exec_pins.iter() {
            pins.push(e.value().clone() as Arc<dyn ExecPin>);
        }
        pins
    }
    fn get_input(&self, name: &str) -> Option<Arc<dyn InDataPin>> {
        self.get_input_by_name(name)
            .map(|p| p as Arc<dyn InDataPin>)
    }
    fn get_output(&self, name: &str) -> Option<Arc<dyn OutDataPin>> {
        self.get_output_by_name(name)
            .map(|p| p as Arc<dyn OutDataPin>)
    }
    fn get_exec_pin(&self, name: &str) -> Option<Arc<dyn ExecPin>> {
        self.get_in_exec_pin_by_name(name)
            .map(|p| p as Arc<dyn ExecPin>)
            .or_else(|| {
                self.get_out_exec_pin_by_name(name)
                    .map(|p| p as Arc<dyn ExecPin>)
            })
    }
}

impl std::fmt::Debug for GenericNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericNode")
            .field("node_type", &self.node_type)
            .finish()
    }
}

// ==================== 动态 Pin 支持的辅助实现 ====================

impl DynamicPinType {
    pub fn matches(&self, type_str: &str) -> bool {
        match (self, type_str) {
            (DynamicPinType::Exec, "exec") => true,
            (DynamicPinType::Data, "data") => true,
            _ => false,
        }
    }
}
