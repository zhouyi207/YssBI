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

    // 使用 DashMap 支持并发访问（使用 PinId 作为键）
    in_data_pins: DashMap<PinId, Arc<GenericInDataPin>>,
    out_data_pins: DashMap<PinId, Arc<GenericOutDataPin>>,
    in_exec_pins: DashMap<PinId, Arc<GenericInExecPin>>,
    out_exec_pins: DashMap<PinId, Arc<GenericOutExecPin>>,

    // 执行器逻辑
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
            in_data_pins: DashMap::new(),
            out_data_pins: DashMap::new(),
            in_exec_pins: DashMap::new(),
            out_exec_pins: DashMap::new(),
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
            in_data_pins: DashMap::new(),
            out_data_pins: DashMap::new(),
            in_exec_pins: DashMap::new(),
            out_exec_pins: DashMap::new(),
            flow_processor: Mutex::new(None),
            data_processor: Mutex::new(None),
        }
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
        pin
    }

    pub fn add_output(&self, pin: GenericOutDataPin) -> Arc<GenericOutDataPin> {
        let id = pin.id();
        let pin = Arc::new(pin);
        self.out_data_pins.insert(id, pin.clone());
        pin
    }

    pub fn add_in_exec_pin(&self, pin: GenericInExecPin) -> Arc<GenericInExecPin> {
        let id = pin.id();
        let pin = Arc::new(pin);
        self.in_exec_pins.insert(id, pin.clone());
        pin
    }

    pub fn add_out_exec_pin(&self, pin: GenericOutExecPin) -> Arc<GenericOutExecPin> {
        let id = pin.id();
        let pin = Arc::new(pin);
        self.out_exec_pins.insert(id, pin.clone());
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

        // 1. 收集输入执行 Pin
        for entry in self.in_exec_pins.iter() {
            inputs.push(PinDefProxy {
                name: entry.value().name().to_string(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            });
        }

        // 2. 收集输出执行 Pin
        for entry in self.out_exec_pins.iter() {
            outputs.push(PinDefProxy {
                name: entry.value().name().to_string(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            });
        }

        // 3. 收集数据输入 Pin
        for entry in self.in_data_pins.iter() {
            inputs.push(PinDefProxy {
                name: entry.value().name().to_string(),
                pin_type: entry.value().data_type().to_string(),
                default_value: None,
                is_array: false,
            });
        }

        // 4. 收集数据输出 Pin
        for entry in self.out_data_pins.iter() {
            outputs.push(PinDefProxy {
                name: entry.value().name().to_string(),
                pin_type: entry.value().data_type().to_string(),
                default_value: None,
                is_array: false,
            });
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
        self.in_data_pins
            .iter()
            .map(|e| e.value().clone() as Arc<dyn InDataPin>)
            .collect()
    }
    fn outputs(&self) -> Vec<Arc<dyn OutDataPin>> {
        self.out_data_pins
            .iter()
            .map(|e| e.value().clone() as Arc<dyn OutDataPin>)
            .collect()
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
