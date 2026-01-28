//! Pin 具体实现

use std::any::Any;
use std::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::executor::error::{ExecutionResult, NodeResult};
use crate::executor::types::DataValue;
use crate::executor::node::NodeId;
use super::traits::{BasePin, DataPin, ExecPin, InDataPin, OutDataPin};
use super::types::{DataPinEvent, DataPinState, ExecPinState, PinId};

/// 泛型输入数据 Pin
pub struct GenericInDataPin {
    id: PinId,
    node_id: NodeId,
    name: String,
    data_type: String,
    state: RwLock<DataPinState>,
    value: RwLock<DataValue>,
    upstream: RwLock<Option<PinId>>,
    listeners: Mutex<Vec<Box<dyn Fn(DataPinEvent) + Send + Sync + 'static>>>,
}

impl GenericInDataPin {
    pub fn new(node_id: NodeId, name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            name: name.into(),
            data_type: data_type.into(),
            state: RwLock::new(DataPinState::Uninitialized),
            value: RwLock::new(DataValue::None),
            upstream: RwLock::new(None),
            listeners: Mutex::new(Vec::new()),
        }
    }

    fn emit_event(&self, event: DataPinEvent) {
        if let Ok(listeners) = self.listeners.lock() {
            for listener in listeners.iter() {
                listener(event.clone());
            }
        }
    }
}

impl std::fmt::Debug for GenericInDataPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericInDataPin")
            .field("id", &self.id)
            .field("node_id", &self.node_id)
            .field("name", &self.name)
            .field("data_type", &self.data_type)
            .field("state", &self.state)
            .field("value", &self.value)
            .field("upstream", &self.upstream)
            .field("listeners", &format!("<{} listeners>", self.listeners.lock().map(|l| l.len()).unwrap_or(0)))
            .finish()
    }
}

impl BasePin for GenericInDataPin {
    fn id(&self) -> PinId {
        self.id
    }

    fn node_id(&self) -> NodeId {
        self.node_id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl DataPin for GenericInDataPin {
    fn value(&self) -> DataValue {
        self.value.read().unwrap().clone()
    }

    fn set_value(&mut self, value: DataValue) -> NodeResult<()> {
        *self.value.write().unwrap() = value.clone();
        self.set_state(DataPinState::Ready);
        
        self.emit_event(DataPinEvent::DataUpdated {
            pin_id: self.id,
            value,
        });
        
        Ok(())
    }

    fn state(&self) -> DataPinState {
        *self.state.read().unwrap()
    }

    fn set_state(&mut self, new_state: DataPinState) {
        let old_state = *self.state.read().unwrap();
        *self.state.write().unwrap() = new_state;
        
        if old_state != new_state {
            self.emit_event(DataPinEvent::StateChanged {
                pin_id: self.id,
                old_state,
                new_state,
            });
        }
    }

    fn data_type(&self) -> &str {
        &self.data_type
    }

    fn subscribe(&mut self, callback: Box<dyn Fn(DataPinEvent) + Send + Sync + 'static>) {
        if let Ok(mut listeners) = self.listeners.lock() {
            listeners.push(callback);
        }
    }
}

impl InDataPin for GenericInDataPin {
    fn link_to(&mut self, out_pin_id: PinId) -> NodeResult<()> {
        *self.upstream.write().unwrap() = Some(out_pin_id);
        
        self.emit_event(DataPinEvent::Connected {
            from_pin: out_pin_id,
            to_pin: self.id,
        });
        
        Ok(())
    }

    fn unlink(&mut self) -> NodeResult<()> {
        if let Some(upstream_id) = *self.upstream.read().unwrap() {
            *self.upstream.write().unwrap() = None;
            
            self.emit_event(DataPinEvent::Disconnected {
                from_pin: upstream_id,
                to_pin: self.id,
            });
        }
        
        Ok(())
    }

    fn upstream(&self) -> Option<PinId> {
        *self.upstream.read().unwrap()
    }

    fn read_from_upstream(&mut self) -> NodeResult<DataValue> {
        Ok(self.value())
    }
}

/// 泛型输出数据 Pin
pub struct GenericOutDataPin {
    id: PinId,
    node_id: NodeId,
    name: String,
    data_type: String,
    state: RwLock<DataPinState>,
    value: RwLock<DataValue>,
    downstream: RwLock<Vec<PinId>>,
    listeners: Mutex<Vec<Box<dyn Fn(DataPinEvent) + Send + Sync + 'static>>>,
    error_message: RwLock<Option<String>>,
}

impl GenericOutDataPin {
    pub fn new(node_id: NodeId, name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            name: name.into(),
            data_type: data_type.into(),
            state: RwLock::new(DataPinState::Uninitialized),
            value: RwLock::new(DataValue::None),
            downstream: RwLock::new(Vec::new()),
            listeners: Mutex::new(Vec::new()),
            error_message: RwLock::new(None),
        }
    }

    fn emit_event(&self, event: DataPinEvent) {
        if let Ok(listeners) = self.listeners.lock() {
            for listener in listeners.iter() {
                listener(event.clone());
            }
        }
    }
}

impl std::fmt::Debug for GenericOutDataPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericOutDataPin")
            .field("id", &self.id)
            .field("node_id", &self.node_id)
            .field("name", &self.name)
            .field("data_type", &self.data_type)
            .field("state", &self.state)
            .field("value", &self.value)
            .field("downstream", &self.downstream)
            .field("listeners", &format!("<{} listeners>", self.listeners.lock().map(|l| l.len()).unwrap_or(0)))
            .field("error_message", &self.error_message)
            .finish()
    }
}

impl BasePin for GenericOutDataPin {
    fn id(&self) -> PinId {
        self.id
    }

    fn node_id(&self) -> NodeId {
        self.node_id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl DataPin for GenericOutDataPin {
    fn value(&self) -> DataValue {
        self.value.read().unwrap().clone()
    }

    fn set_value(&mut self, value: DataValue) -> NodeResult<()> {
        *self.value.write().unwrap() = value.clone();
        self.set_state(DataPinState::Ready);
        
        self.emit_event(DataPinEvent::DataUpdated {
            pin_id: self.id,
            value,
        });
        
        Ok(())
    }

    fn state(&self) -> DataPinState {
        *self.state.read().unwrap()
    }

    fn set_state(&mut self, new_state: DataPinState) {
        let old_state = *self.state.read().unwrap();
        *self.state.write().unwrap() = new_state;
        
        if old_state != new_state {
            self.emit_event(DataPinEvent::StateChanged {
                pin_id: self.id,
                old_state,
                new_state,
            });
        }
    }

    fn data_type(&self) -> &str {
        &self.data_type
    }

    fn subscribe(&mut self, callback: Box<dyn Fn(DataPinEvent) + Send + Sync + 'static>) {
        if let Ok(mut listeners) = self.listeners.lock() {
            listeners.push(callback);
        }
    }
}

impl OutDataPin for GenericOutDataPin {
    fn write(&mut self, data: DataValue) -> NodeResult<()> {
        self.set_value(data)
    }

    fn set_error(&mut self, message: String) {
        *self.error_message.write().unwrap() = Some(message);
        self.set_state(DataPinState::Error);
    }

    fn reset(&mut self) {
        *self.value.write().unwrap() = DataValue::None;
        *self.error_message.write().unwrap() = None;
        self.set_state(DataPinState::Uninitialized);
    }

    fn downstream(&self) -> Vec<PinId> {
        self.downstream.read().unwrap().clone()
    }

    fn add_downstream(&mut self, in_pin_id: PinId) -> NodeResult<()> {
        let mut downstream = self.downstream.write().unwrap();
        if !downstream.contains(&in_pin_id) {
            downstream.push(in_pin_id);
            
            self.emit_event(DataPinEvent::Connected {
                from_pin: self.id,
                to_pin: in_pin_id,
            });
        }
        Ok(())
    }

    fn remove_downstream(&mut self, in_pin_id: PinId) -> NodeResult<()> {
        let mut downstream = self.downstream.write().unwrap();
        if let Some(pos) = downstream.iter().position(|&id| id == in_pin_id) {
            downstream.remove(pos);
            
            self.emit_event(DataPinEvent::Disconnected {
                from_pin: self.id,
                to_pin: in_pin_id,
            });
        }
        Ok(())
    }
}

/// 泛型执行 Pin
pub struct GenericExecPin {
    id: PinId,
    node_id: NodeId,
    name: String,
    state: RwLock<ExecPinState>,
    dependencies: RwLock<Vec<PinId>>,
    next_pin: RwLock<Option<PinId>>,
    executor: Mutex<Option<Box<dyn Fn() -> ExecutionResult<()> + Send + Sync + 'static>>>,
}

impl GenericExecPin {
    pub fn new(node_id: NodeId, name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            name: name.into(),
            state: RwLock::new(ExecPinState::Idle),
            dependencies: RwLock::new(Vec::new()),
            next_pin: RwLock::new(None),
            executor: Mutex::new(None),
        }
    }

    pub fn set_executor(&mut self, executor: Box<dyn Fn() -> ExecutionResult<()> + Send + Sync + 'static>) {
        *self.executor.lock().unwrap() = Some(executor);
    }
}

impl std::fmt::Debug for GenericExecPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericExecPin")
            .field("id", &self.id)
            .field("node_id", &self.node_id)
            .field("name", &self.name)
            .field("state", &self.state)
            .field("dependencies", &self.dependencies)
            .field("next_pin", &self.next_pin)
            .field("executor", &format!("<{}>", if self.executor.lock().unwrap().is_some() { "Some" } else { "None" }))
            .finish()
    }
}

impl BasePin for GenericExecPin {
    fn id(&self) -> PinId {
        self.id
    }

    fn node_id(&self) -> NodeId {
        self.node_id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl ExecPin for GenericExecPin {
    fn trigger(&mut self) -> ExecutionResult<()> {
        // 检查状态
        let current_state = self.state();
        if current_state != ExecPinState::Idle && current_state != ExecPinState::Blocked {
            return Err(crate::executor::error::ExecutionError::Generic(
                format!("执行 Pin 状态不正确：{:?}", current_state)
            ));
        }

        // 设置状态为 Running
        self.set_state(ExecPinState::Running);

        // 执行
        let result = if let Some(executor) = self.executor.lock().unwrap().as_ref() {
            executor()
        } else {
            Ok(())
        };

        // 更新状态
        match result {
            Ok(_) => self.set_state(ExecPinState::Completed),
            Err(_) => self.set_state(ExecPinState::Failed),
        }

        result
    }

    fn state(&self) -> ExecPinState {
        *self.state.read().unwrap()
    }

    fn set_state(&mut self, state: ExecPinState) {
        *self.state.write().unwrap() = state;
    }

    fn add_dependency(&mut self, pin_id: PinId) -> NodeResult<()> {
        let mut deps = self.dependencies.write().unwrap();
        if !deps.contains(&pin_id) {
            deps.push(pin_id);
        }
        Ok(())
    }

    fn remove_dependency(&mut self, pin_id: PinId) -> NodeResult<()> {
        let mut deps = self.dependencies.write().unwrap();
        if let Some(pos) = deps.iter().position(|&id| id == pin_id) {
            deps.remove(pos);
        }
        Ok(())
    }

    fn check_dependencies_ready(&self) -> bool {
        true
    }

    fn connect_to(&mut self, next_pin_id: PinId) -> NodeResult<()> {
        *self.next_pin.write().unwrap() = Some(next_pin_id);
        Ok(())
    }

    fn next(&self) -> Option<PinId> {
        *self.next_pin.read().unwrap()
    }
}
