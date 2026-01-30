//! 节点实现模块
//!
//! 实现 GenericNode：通用的节点容器

use dashmap::DashMap;
use serde::{ser::SerializeStruct, Serialize, Serializer};
use serde_json::Value;
use std::sync::{Arc, Mutex, RwLock};

use super::traits::Node;
use super::types::{NodeId, NodeState};
use crate::executor::error::{ExecutionResult, NodeResult};
use crate::executor::node::NodeData;
use crate::executor::pin::{
    BasePin, DataPin, ExecPin, GenericInDataPin, GenericInExecPin, GenericOutDataPin,
    GenericOutExecPin, InDataPin, OutDataPin, PinId,
};
use crate::executor::processors::ExecutionContextTrait;

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
