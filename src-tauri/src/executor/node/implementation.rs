//! 节点实现模块
//!
//! 实现 GenericNode：通用的节点容器

use std::sync::{Arc, Mutex, RwLock};
use dashmap::DashMap;
use serde::{Serialize, Serializer, ser::SerializeStruct};
use serde_json::Value;

<<<<<<< HEAD
use crate::executor::error::{ExecutionResult, NodeResult};
use crate::executor::pin::{GenericInExecPin, GenericOutExecPin, GenericInDataPin, GenericOutDataPin, BasePin, ExecPin, InDataPin, OutDataPin, DataPin};
=======
<<<<<<< HEAD
use crate::executor::error::{ExecutionResult, NodeResult, NodeError};
use crate::executor::pin::{GenericExecPin, GenericInDataPin, GenericOutDataPin, BasePin, ExecPin, InDataPin, OutDataPin, ExecPinState, DataPin};
=======
use crate::executor::error::{ExecutionResult, NodeResult};
use crate::executor::pin::{GenericInExecPin, GenericOutExecPin, GenericInDataPin, GenericOutDataPin, BasePin, ExecPin, InDataPin, OutDataPin, DataPin};
>>>>>>> 9639c93 (feat: implement initial node-based executor system including graph management, execution context, and various node types like variable, debug, control, function, internal, and visualization.)
>>>>>>> e76e66554c9d5f618cb80ed5ecc399c15510152e
use super::traits::Node;
use super::types::{NodeId, NodeState};
use crate::executor::node::NodeData;
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
    
    // 使用 DashMap 支持并发访问
    in_data_pins: DashMap<String, Arc<GenericInDataPin>>,
    out_data_pins: DashMap<String, Arc<GenericOutDataPin>>,
    in_exec_pins: DashMap<String, Arc<GenericInExecPin>>,
    out_exec_pins: DashMap<String, Arc<GenericOutExecPin>>,
    
    // 执行器逻辑
    flow_processor: Mutex<Option<Box<dyn Fn(&mut dyn ExecutionContextTrait, &NodeData) -> Result<String, String> + Send + Sync + 'static>>>,
    data_processor: Mutex<Option<Box<dyn Fn(&mut dyn ExecutionContextTrait, &NodeData, &str) -> Value + Send + Sync + 'static>>>,
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

    pub fn set_metadata(&mut self, category: Vec<String>, ui_style: String, description: Option<String>) {
        self.category = category;
        self.ui_style = ui_style;
        self.description = description;
    }

    pub fn add_input(&self, pin: GenericInDataPin) -> Arc<GenericInDataPin> {
        let name = pin.name().to_string();
        let pin = Arc::new(pin);
        self.in_data_pins.insert(name, pin.clone());
        pin
    }

    pub fn add_output(&self, pin: GenericOutDataPin) -> Arc<GenericOutDataPin> {
        let name = pin.name().to_string();
        let pin = Arc::new(pin);
        self.out_data_pins.insert(name, pin.clone());
        pin
    }

    pub fn add_in_exec_pin(&self, pin: GenericInExecPin) -> Arc<GenericInExecPin> {
        let name = pin.name().to_string();
        let pin = Arc::new(pin);
        self.in_exec_pins.insert(name, pin.clone());
        pin
    }

    pub fn add_out_exec_pin(&self, pin: GenericOutExecPin) -> Arc<GenericOutExecPin> {
        let name = pin.name().to_string();
        let pin = Arc::new(pin);
        self.out_exec_pins.insert(name, pin.clone());
        pin
    }

    // 向后兼容：add_exec_pin 作为输出 exec pin（大多数节点使用）
    pub fn add_exec_pin(&self, pin: GenericOutExecPin) -> Arc<GenericOutExecPin> {
        self.add_out_exec_pin(pin)
    }    pub fn set_flow_processor(&self, processor: Box<dyn Fn(&mut dyn ExecutionContextTrait, &NodeData) -> Result<String, String> + Send + Sync + 'static>) {
        *self.flow_processor.lock().unwrap() = Some(processor);
    }

    pub fn set_data_processor(&self, processor: Box<dyn Fn(&mut dyn ExecutionContextTrait, &NodeData, &str) -> Value + Send + Sync + 'static>) {
        *self.data_processor.lock().unwrap() = Some(processor);
    }

    pub fn process_flow(&self, ctx: &mut dyn ExecutionContextTrait, node: &NodeData) -> Result<String, String> {
        if let Some(p) = self.flow_processor.lock().unwrap().as_ref() {
            p(ctx, node)
        } else { Ok("".into()) }
    }

    pub fn process_data(&self, ctx: &mut dyn ExecutionContextTrait, node: &NodeData, pin_id: &str) -> Value {
        if let Some(p) = self.data_processor.lock().unwrap().as_ref() {
            p(ctx, node, pin_id)
        } else { Value::Null }
    }

<<<<<<< HEAD
=======
<<<<<<< HEAD
    pub fn node_type(&self) -> &str {
        &self.node_type
    }
=======
>>>>>>> e76e66554c9d5f618cb80ed5ecc399c15510152e
    pub fn node_type(&self) -> &str { &self.node_type }
    pub fn input_names(&self) -> Vec<String> { self.in_data_pins.iter().map(|e| e.key().clone()).collect() }
    pub fn output_names(&self) -> Vec<String> { self.out_data_pins.iter().map(|e| e.key().clone()).collect() }
    pub fn get_input_concrete(&self, name: &str) -> Option<Arc<GenericInDataPin>> { self.in_data_pins.get(name).map(|p| p.clone()) }
    pub fn get_output_concrete(&self, name: &str) -> Option<Arc<GenericOutDataPin>> { self.out_data_pins.get(name).map(|p| p.clone()) }
}
<<<<<<< HEAD
=======
>>>>>>> 9639c93 (feat: implement initial node-based executor system including graph management, execution context, and various node types like variable, debug, control, function, internal, and visualization.)
>>>>>>> e76e66554c9d5f618cb80ed5ecc399c15510152e

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

<<<<<<< HEAD
=======
<<<<<<< HEAD
        for entry in self.exec_pins.iter() {
            let pin = entry.value();
            let state = pin.state();
            
            match state {
                ExecPinState::Running => has_running = true,
                ExecPinState::Failed => has_failed = true,
                ExecPinState::Blocked => has_blocked = true,
                ExecPinState::Completed => {},
                _ => all_completed = false,
            }
=======
>>>>>>> e76e66554c9d5f618cb80ed5ecc399c15510152e
        // 1. 收集输入执行 Pin
        for entry in self.in_exec_pins.iter() {
            inputs.push(PinDefProxy {
                name: entry.key().clone(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            });
<<<<<<< HEAD
=======
        }

        // 2. 收集输出执行 Pin
        for entry in self.out_exec_pins.iter() {
            outputs.push(PinDefProxy {
                name: entry.key().clone(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            });
        }

        // 3. 收集数据输入 Pin
        for entry in self.in_data_pins.iter() {
            inputs.push(PinDefProxy {
                name: entry.key().clone(),
                pin_type: entry.value().data_type().to_string(),
                default_value: None,
                is_array: false,
            });
        }

        // 4. 收集数据输出 Pin
        for entry in self.out_data_pins.iter() {
            outputs.push(PinDefProxy {
                name: entry.key().clone(),
                pin_type: entry.value().data_type().to_string(),
                default_value: None,
                is_array: false,
            });
>>>>>>> 9639c93 (feat: implement initial node-based executor system including graph management, execution context, and various node types like variable, debug, control, function, internal, and visualization.)
>>>>>>> e76e66554c9d5f618cb80ed5ecc399c15510152e
        }

        // 2. 收集输出执行 Pin
        for entry in self.out_exec_pins.iter() {
            outputs.push(PinDefProxy {
                name: entry.key().clone(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            });
        }

        // 3. 收集数据输入 Pin
        for entry in self.in_data_pins.iter() {
            inputs.push(PinDefProxy {
                name: entry.key().clone(),
                pin_type: entry.value().data_type().to_string(),
                default_value: None,
                is_array: false,
            });
        }

        // 4. 收集数据输出 Pin
        for entry in self.out_data_pins.iter() {
            outputs.push(PinDefProxy {
                name: entry.key().clone(),
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
    fn id(&self) -> NodeId { self.id }
    fn name(&self) -> &str { "GenericNode" }
    fn set_name(&mut self, name: String) { *self.title.write().unwrap() = name; }
    fn state(&self) -> NodeState { *self.state.read().unwrap() }
<<<<<<< HEAD
    fn execute(&mut self) -> ExecutionResult<()> { Ok(()) }
    fn reset(&mut self) -> NodeResult<()> { Ok(()) }
    fn dispose(&mut self) -> NodeResult<()> { Ok(()) }
    fn inputs(&self) -> Vec<Arc<dyn InDataPin>> { self.in_data_pins.iter().map(|e| e.value().clone() as Arc<dyn InDataPin>).collect() }
    fn outputs(&self) -> Vec<Arc<dyn OutDataPin>> { self.out_data_pins.iter().map(|e| e.value().clone() as Arc<dyn OutDataPin>).collect() }
=======
<<<<<<< HEAD

    fn execute(&mut self) -> ExecutionResult<()> {
        self.check_disposed().map_err(|e| crate::executor::error::ExecutionError::Generic(e.to_string()))?;
        *self.state.write().unwrap() = NodeState::Running;
        
        // 注意：这里的 execution 是针对 GenericNode 实例本身的，
        // 而不是通过 ExecutionContextTrait 的。
        // 为了兼容性，我们暂时不做太多改变。
        
        self.update_state();
        Ok(())
    }

    fn reset(&mut self) -> NodeResult<()> { 
        self.check_disposed()?;
        for mut entry in self.inputs.iter_mut() {
            let pin = entry.value_mut();
            pin.unlink()?;
        }
        for mut entry in self.outputs.iter_mut() {
            let pin = entry.value_mut();
            pin.reset();
        }
        for mut entry in self.exec_pins.iter_mut() {
            let pin = entry.value_mut();
            pin.set_state(ExecPinState::Idle);
        }
        *self.state.write().unwrap() = NodeState::Idle;
        Ok(())
    }
    
    fn dispose(&mut self) -> NodeResult<()> {
        self.check_disposed()?;
        self.inputs.clear(); self.outputs.clear(); self.exec_pins.clear();
        *self.state.write().unwrap() = NodeState::Disposed;
        Ok(())
    }

    fn inputs(&self) -> Vec<Arc<dyn InDataPin>> {
        self.inputs.iter().map(|e| e.value().clone() as Arc<dyn InDataPin>).collect()
    }
    fn outputs(&self) -> Vec<Arc<dyn OutDataPin>> {
        self.outputs.iter().map(|e| e.value().clone() as Arc<dyn OutDataPin>).collect()
    }
>>>>>>> e76e66554c9d5f618cb80ed5ecc399c15510152e
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
    fn get_input(&self, name: &str) -> Option<Arc<dyn InDataPin>> { self.in_data_pins.get(name).map(|p| p.clone() as Arc<dyn InDataPin>) }
    fn get_output(&self, name: &str) -> Option<Arc<dyn OutDataPin>> { self.out_data_pins.get(name).map(|p| p.clone() as Arc<dyn OutDataPin>) }
    fn get_exec_pin(&self, name: &str) -> Option<Arc<dyn ExecPin>> {
<<<<<<< HEAD
        self.in_exec_pins.get(name)
            .map(|p| p.clone() as Arc<dyn ExecPin>)
            .or_else(|| self.out_exec_pins.get(name).map(|p| p.clone() as Arc<dyn ExecPin>))
=======
        self.exec_pins.get(name).map(|p| p.clone() as Arc<dyn ExecPin>)
=======
    fn execute(&mut self) -> ExecutionResult<()> { Ok(()) }
    fn reset(&mut self) -> NodeResult<()> { Ok(()) }
    fn dispose(&mut self) -> NodeResult<()> { Ok(()) }
    fn inputs(&self) -> Vec<Arc<dyn InDataPin>> { self.in_data_pins.iter().map(|e| e.value().clone() as Arc<dyn InDataPin>).collect() }
    fn outputs(&self) -> Vec<Arc<dyn OutDataPin>> { self.out_data_pins.iter().map(|e| e.value().clone() as Arc<dyn OutDataPin>).collect() }
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
    fn get_input(&self, name: &str) -> Option<Arc<dyn InDataPin>> { self.in_data_pins.get(name).map(|p| p.clone() as Arc<dyn InDataPin>) }
    fn get_output(&self, name: &str) -> Option<Arc<dyn OutDataPin>> { self.out_data_pins.get(name).map(|p| p.clone() as Arc<dyn OutDataPin>) }
    fn get_exec_pin(&self, name: &str) -> Option<Arc<dyn ExecPin>> {
        self.in_exec_pins.get(name)
            .map(|p| p.clone() as Arc<dyn ExecPin>)
            .or_else(|| self.out_exec_pins.get(name).map(|p| p.clone() as Arc<dyn ExecPin>))
>>>>>>> 9639c93 (feat: implement initial node-based executor system including graph management, execution context, and various node types like variable, debug, control, function, internal, and visualization.)
>>>>>>> e76e66554c9d5f618cb80ed5ecc399c15510152e
    }
}

impl std::fmt::Debug for GenericNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericNode").field("node_type", &self.node_type).finish()
    }
}
