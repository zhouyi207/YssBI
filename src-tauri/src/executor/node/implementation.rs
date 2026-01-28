//! 节点实现模块
//!
//! 实现 GenericNode：通用的节点容器

use std::sync::{Arc, Mutex, RwLock};
use dashmap::DashMap;
use serde_json::Value;

use crate::executor::error::{ExecutionResult, NodeResult, NodeError};
use crate::executor::pin::{GenericExecPin, GenericInDataPin, GenericOutDataPin, BasePin, ExecPin, InDataPin, OutDataPin, ExecPinState, DataPin};
use super::traits::Node;
use super::types::{NodeId, NodeState};
use crate::executor::node::definition::NodeDefinition;
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
    inputs: DashMap<String, Arc<GenericInDataPin>>,
    outputs: DashMap<String, Arc<GenericOutDataPin>>,
    exec_pins: DashMap<String, Arc<GenericExecPin>>,
    
    // 执行器：节点的业务逻辑
    // 流程处理逻辑
    flow_processor: Mutex<Option<Box<dyn Fn(&mut dyn ExecutionContextTrait, &NodeData) -> Result<String, String> + Send + Sync + 'static>>>,
    // 数据计算逻辑
    data_processor: Mutex<Option<Box<dyn Fn(&mut dyn ExecutionContextTrait, &NodeData, &str) -> Value + Send + Sync + 'static>>>,
}

impl GenericNode {
    /// 创建新节点原型（用于注册）
    pub fn new_prototype(node_type: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::nil(),
            title: RwLock::new(title.into()),
            node_type: node_type.into(),
            state: RwLock::new(NodeState::Idle),
            category: Vec::new(),
            ui_style: "default".into(),
            description: None,
            inputs: DashMap::new(),
            outputs: DashMap::new(),
            exec_pins: DashMap::new(),
            flow_processor: Mutex::new(None),
            data_processor: Mutex::new(None),
        }
    }

    /// 创建带 ID 的节点实例
    pub fn new(id: NodeId, title: impl Into<String>, node_type: impl Into<String>) -> Self {
        Self {
            id,
            title: RwLock::new(title.into()),
            node_type: node_type.into(),
            state: RwLock::new(NodeState::Idle),
            category: Vec::new(),
            ui_style: "default".into(),
            description: None,
            inputs: DashMap::new(),
            outputs: DashMap::new(),
            exec_pins: DashMap::new(),
            flow_processor: Mutex::new(None),
            data_processor: Mutex::new(None),
        }
    }

    /// 设置元数据
    pub fn set_metadata(&mut self, category: Vec<String>, ui_style: String, description: Option<String>) {
        self.category = category;
        self.ui_style = ui_style;
        self.description = description;
    }

    /// 获取 NodeDefinition 格式的序列化信息 (用于前端)
    pub fn to_definition(&self) -> NodeDefinition {
        use crate::executor::node::data::PinDefinition;
        
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();

        // 收集执行 Pin
        for entry in self.exec_pins.iter() {
            inputs.push(PinDefinition {
                name: entry.key().clone(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            });
        }

        // 收集数据输入
        for entry in self.inputs.iter() {
            let pin = entry.value();
            inputs.push(PinDefinition {
                name: entry.key().clone(),
                pin_type: pin.data_type().to_string(),
                default_value: None,
                is_array: false,
            });
        }

        // 收集数据输出
        for entry in self.outputs.iter() {
            let pin = entry.value();
            outputs.push(PinDefinition {
                name: entry.key().clone(),
                pin_type: pin.data_type().to_string(),
                default_value: None,
                is_array: false,
            });
        }

        NodeDefinition {
            node_type: self.node_type.clone(),
            category: self.category.clone(),
            title: self.title.read().unwrap().clone(),
            inputs,
            outputs,
            ui_style: self.ui_style.clone(),
            description: self.description.clone(),
            data_processor: None,
            flow_processor: None,
        }
    }

    pub fn add_input(&self, pin: GenericInDataPin) -> Arc<GenericInDataPin> {
        let name = pin.name().to_string();
        let pin = Arc::new(pin);
        self.inputs.insert(name, pin.clone());
        pin
    }

    pub fn add_output(&self, pin: GenericOutDataPin) -> Arc<GenericOutDataPin> {
        let name = pin.name().to_string();
        let pin = Arc::new(pin);
        self.outputs.insert(name, pin.clone());
        pin
    }

    pub fn add_exec_pin(&self, pin: GenericExecPin) -> Arc<GenericExecPin> {
        let name = pin.name().to_string();
        let pin = Arc::new(pin);
        self.exec_pins.insert(name, pin.clone());
        pin
    }

    pub fn set_flow_processor(&self, processor: Box<dyn Fn(&mut dyn ExecutionContextTrait, &NodeData) -> Result<String, String> + Send + Sync + 'static>) {
        *self.flow_processor.lock().unwrap() = Some(processor);
    }

    pub fn set_data_processor(&self, processor: Box<dyn Fn(&mut dyn ExecutionContextTrait, &NodeData, &str) -> Value + Send + Sync + 'static>) {
        *self.data_processor.lock().unwrap() = Some(processor);
    }

    pub fn process_flow(&self, ctx: &mut dyn ExecutionContextTrait, node: &NodeData) -> Result<String, String> {
        if let Some(p) = self.flow_processor.lock().unwrap().as_ref() {
            p(ctx, node)
        } else {
            Ok("".into())
        }
    }

    pub fn process_data(&self, ctx: &mut dyn ExecutionContextTrait, node: &NodeData, pin_id: &str) -> Value {
        if let Some(p) = self.data_processor.lock().unwrap().as_ref() {
            p(ctx, node, pin_id)
        } else {
            Value::Null
        }
    }

    pub fn node_type(&self) -> &str {
        &self.node_type
    }

    fn check_disposed(&self) -> NodeResult<()> {
        if *self.state.read().unwrap() == NodeState::Disposed {
            return Err(NodeError::NodeDisposed(self.id));
        }
        Ok(())
    }

    fn update_state(&self) {
        if self.exec_pins.is_empty() {
            *self.state.write().unwrap() = NodeState::Idle;
            return;
        }

        let mut has_running = false;
        let mut has_failed = false;
        let mut has_blocked = false;
        let mut all_completed = true;

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
        }

        let new_state = if has_running {
            NodeState::Running
        } else if has_failed {
            NodeState::Failed
        } else if has_blocked {
            NodeState::Blocked
        } else if all_completed {
            NodeState::Completed
        } else {
            NodeState::Idle
        };

        *self.state.write().unwrap() = new_state;
    }

    pub fn input_names(&self) -> Vec<String> {
        self.inputs.iter().map(|e| e.key().clone()).collect()
    }

    pub fn output_names(&self) -> Vec<String> {
        self.outputs.iter().map(|e| e.key().clone()).collect()
    }

    pub fn exec_pin_names(&self) -> Vec<String> {
        self.exec_pins.iter().map(|e| e.key().clone()).collect()
    }

    pub fn get_input_concrete(&self, name: &str) -> Option<Arc<GenericInDataPin>> {
        self.inputs.get(name).map(|p| p.clone())
    }

    pub fn get_output_concrete(&self, name: &str) -> Option<Arc<GenericOutDataPin>> {
        self.outputs.get(name).map(|p| p.clone())
    }

    pub fn get_exec_pin_concrete(&self, name: &str) -> Option<Arc<GenericExecPin>> {
        self.exec_pins.get(name).map(|p| p.clone())
    }
}

impl Node for GenericNode {
    fn id(&self) -> NodeId { self.id }
    fn name(&self) -> &str { "GenericNode" }
    fn set_name(&mut self, name: String) { *self.title.write().unwrap() = name; }
    fn state(&self) -> NodeState { *self.state.read().unwrap() }

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
    fn exec_pins(&self) -> Vec<Arc<dyn ExecPin>> {
        self.exec_pins.iter().map(|e| e.value().clone() as Arc<dyn ExecPin>).collect()
    }
    fn get_input(&self, name: &str) -> Option<Arc<dyn InDataPin>> {
        self.inputs.get(name).map(|p| p.clone() as Arc<dyn InDataPin>)
    }
    fn get_output(&self, name: &str) -> Option<Arc<dyn OutDataPin>> {
        self.outputs.get(name).map(|p| p.clone() as Arc<dyn OutDataPin>)
    }
    fn get_exec_pin(&self, name: &str) -> Option<Arc<dyn ExecPin>> {
        self.exec_pins.get(name).map(|p| p.clone() as Arc<dyn ExecPin>)
    }
}

impl std::fmt::Debug for GenericNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericNode")
            .field("node_type", &self.node_type)
            .field("title", &self.title)
            .finish()
    }
}
