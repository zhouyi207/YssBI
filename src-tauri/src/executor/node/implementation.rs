//! 节点实现模块
//!
//! 实现 GenericNode：通用的节点容器

use std::sync::{Arc, Mutex, RwLock};
use dashmap::DashMap;

use crate::executor::error::{ExecutionResult, NodeResult, NodeError};
use crate::executor::pin::{GenericExecPin, GenericInDataPin, GenericOutDataPin, BasePin, ExecPin, InDataPin, OutDataPin, ExecPinState};
use super::traits::Node;
use super::types::{NodeId, NodeState};

/// 泛型节点实现
///
/// 使用 DashMap 存储 Pin，支持并发访问
pub struct GenericNode {
    id: NodeId,
    name: RwLock<String>,
    node_type: String,
    state: RwLock<NodeState>,
    
    // 使用 DashMap 支持并发访问
    inputs: DashMap<String, Arc<Mutex<GenericInDataPin>>>,
    outputs: DashMap<String, Arc<Mutex<GenericOutDataPin>>>,
    exec_pins: DashMap<String, Arc<Mutex<GenericExecPin>>>,
    
    // 执行器：节点的业务逻辑
    processor: Mutex<Option<Box<dyn Fn(&GenericNode) -> ExecutionResult<()> + Send + Sync + 'static>>>,
}

impl GenericNode {
    /// 创建新节点
    pub fn new(id: NodeId, name: impl Into<String>, node_type: impl Into<String>) -> Self {
        Self {
            id,
            name: RwLock::new(name.into()),
            node_type: node_type.into(),
            state: RwLock::new(NodeState::Idle),
            inputs: DashMap::new(),
            outputs: DashMap::new(),
            exec_pins: DashMap::new(),
            processor: Mutex::new(None),
        }
    }

    /// 添加输入 Pin
    pub fn add_input(&self, pin: GenericInDataPin) -> Arc<Mutex<GenericInDataPin>> {
        let name = pin.name().to_string();
        let pin = Arc::new(Mutex::new(pin));
        self.inputs.insert(name, pin.clone());
        pin
    }

    /// 添加输出 Pin
    pub fn add_output(&self, pin: GenericOutDataPin) -> Arc<Mutex<GenericOutDataPin>> {
        let name = pin.name().to_string();
        let pin = Arc::new(Mutex::new(pin));
        self.outputs.insert(name, pin.clone());
        pin
    }

    /// 添加执行 Pin
    pub fn add_exec_pin(&self, pin: GenericExecPin) -> Arc<Mutex<GenericExecPin>> {
        let name = pin.name().to_string();
        let pin = Arc::new(Mutex::new(pin));
        self.exec_pins.insert(name, pin.clone());
        pin
    }

    /// 设置处理器（业务逻辑）
    pub fn set_processor(&self, processor: Box<dyn Fn(&GenericNode) -> ExecutionResult<()> + Send + Sync + 'static>) {
        *self.processor.lock().unwrap() = Some(processor);
    }

    /// 获取节点类型
    pub fn node_type(&self) -> &str {
        &self.node_type
    }

    /// 检查节点是否已销毁
    fn check_disposed(&self) -> NodeResult<()> {
        if *self.state.read().unwrap() == NodeState::Disposed {
            return Err(NodeError::NodeDisposed(self.id));
        }
        Ok(())
    }

    /// 更新节点状态（根据所有执行 Pin 的状态）
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
            if let Ok(pin) = entry.value().lock() {
                let state = pin.state();
                
                match state {
                    ExecPinState::Running => has_running = true,
                    ExecPinState::Failed => has_failed = true,
                    ExecPinState::Blocked => has_blocked = true,
                    ExecPinState::Completed => {},
                    _ => all_completed = false,
                }
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

    /// 获取所有输入 Pin 的名称列表
    pub fn input_names(&self) -> Vec<String> {
        self.inputs.iter().map(|e| e.key().clone()).collect()
    }

    /// 获取所有输出 Pin 的名称列表
    pub fn output_names(&self) -> Vec<String> {
        self.outputs.iter().map(|e| e.key().clone()).collect()
    }

    /// 获取所有执行 Pin 的名称列表
    pub fn exec_pin_names(&self) -> Vec<String> {
        self.exec_pins.iter().map(|e| e.key().clone()).collect()
    }

    /// 获取输入 Pin（返回具体类型，不是 trait object）
    pub fn get_input_concrete(&self, name: &str) -> Option<Arc<Mutex<GenericInDataPin>>> {
        self.inputs.get(name).map(|p| p.clone())
    }

    /// 获取输出 Pin（返回具体类型，不是 trait object）
    pub fn get_output_concrete(&self, name: &str) -> Option<Arc<Mutex<GenericOutDataPin>>> {
        self.outputs.get(name).map(|p| p.clone())
    }

    /// 获取执行 Pin（返回具体类型，不是 trait object）
    pub fn get_exec_pin_concrete(&self, name: &str) -> Option<Arc<Mutex<GenericExecPin>>> {
        self.exec_pins.get(name).map(|p| p.clone())
    }
}

impl std::fmt::Debug for GenericNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericNode")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("node_type", &self.node_type)
            .field("state", &self.state)
            .field("inputs", &format!("<{} inputs>", self.inputs.len()))
            .field("outputs", &format!("<{} outputs>", self.outputs.len()))
            .field("exec_pins", &format!("<{} exec pins>", self.exec_pins.len()))
            .field("processor", &format!("<{}>", if self.processor.lock().unwrap().is_some() { "Some" } else { "None" }))
            .finish()
    }
}

impl Node for GenericNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn name(&self) -> &str {
        "node"
    }

    fn set_name(&mut self, name: String) {
        *self.name.write().unwrap() = name;
    }

    fn state(&self) -> NodeState {
        *self.state.read().unwrap()
    }

    fn execute(&mut self) -> ExecutionResult<()> {
        self.check_disposed().map_err(|e| {
            crate::executor::error::ExecutionError::Generic(e.to_string())
        })?;

        // 设置状态为 Running
        *self.state.write().unwrap() = NodeState::Running;

        // 执行处理器
        let result = if let Some(processor) = self.processor.lock().unwrap().as_ref() {
            processor(self)
        } else {
            Ok(())
        };

        // 更新状态
        self.update_state();

        result
    }

    fn inputs(&self) -> Vec<Arc<dyn InDataPin>> {
        Vec::new()
    }

    fn outputs(&self) -> Vec<Arc<dyn OutDataPin>> {
        Vec::new()
    }

    fn exec_pins(&self) -> Vec<Arc<dyn ExecPin>> {
        Vec::new()
    }

    fn get_input(&self, _name: &str) -> Option<Arc<dyn InDataPin>> {
        None
    }

    fn get_output(&self, _name: &str) -> Option<Arc<dyn OutDataPin>> {
        None
    }

    fn get_exec_pin(&self, _name: &str) -> Option<Arc<dyn ExecPin>> {
        None
    }

    fn reset(&mut self) -> NodeResult<()> {
        self.check_disposed()?;

        // 重置所有输入 Pin
        for entry in self.inputs.iter_mut() {
            if let Ok(mut pin) = entry.value().lock() {
                pin.unlink()?;
            }
        }

        // 重置所有输出 Pin
        for entry in self.outputs.iter_mut() {
            if let Ok(mut pin) = entry.value().lock() {
                pin.reset();
            }
        }

        // 重置所有执行 Pin
        for entry in self.exec_pins.iter_mut() {
            if let Ok(mut pin) = entry.value().lock() {
                pin.set_state(ExecPinState::Idle);
            }
        }

        // 重置节点状态
        *self.state.write().unwrap() = NodeState::Idle;

        Ok(())
    }

    fn dispose(&mut self) -> NodeResult<()> {
        self.check_disposed()?;

        // 清空所有 Pin
        self.inputs.clear();
        self.outputs.clear();
        self.exec_pins.clear();

        // 设置销毁标记
        *self.state.write().unwrap() = NodeState::Disposed;

        Ok(())
    }
}
