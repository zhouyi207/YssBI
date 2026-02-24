use super::NodeExecutionContextTrait;
use crate::graph::core::GraphRuntime;
use crate::graph::infer::TypeVarId;
use crate::graph::node::{NodeId, NodeInstanceParams};
use crate::graph::pin::{PinId, PinRole};
use crate::graph::value::{DataType, DataValue};
use polars::prelude::{DataFrame, Series};
use std::any::Any;
use std::sync::{Arc, Mutex};

/// 窗口打开请求
pub struct WindowAction {
    pub window_type: String,
    pub data: String,
}

/// 具体的执行上下文实现
pub struct NodeExecutionContext {
    pub node_id: NodeId,
    pub graph: Arc<Mutex<GraphRuntime>>,
    pub logs: Vec<String>,
    pub window_actions: Vec<WindowAction>,
}

impl NodeExecutionContext {
    pub fn new(graph: Arc<Mutex<GraphRuntime>>, node_id: NodeId) -> Self {
        Self {
            node_id,
            graph,
            logs: Vec::new(),
            window_actions: Vec::new(),
        }
    }
}

impl NodeExecutionContextTrait for NodeExecutionContext {
    fn get_input_by_role(&self, role: &PinRole) -> Result<DataValue, String> {
        let graph = self.graph.lock().unwrap();
        let pin_instance = graph
            .get_pin_instance_by_pin_role(self.node_id, role)
            .ok_or_else(|| format!("Input pin_instance with role {:?} not found", role))?;

        if !pin_instance.is_input() {
            return Err(format!("Pin {:?} is not an input", role));
        }

        let data_value = graph.get_pin_data_value_by_pin_id(pin_instance.id)?;
        Ok(data_value)
    }

    fn get_inputs_by_role(&self, role: &PinRole) -> Result<Vec<DataValue>, String> {
        let graph = self.graph.lock().unwrap();
        let pin_instances = graph.get_pin_instances_by_pin_role(self.node_id, role);

        if pin_instances.is_empty() {
            return Err(format!("No input pin_instances with role {:?} found", role));
        }

        let mut values = Vec::new();

        for pin in pin_instances {
            if !pin.is_input() {
                continue;
            }
            values.push(graph.get_pin_data_value_by_pin_id(pin.id)?);
        }

        Ok(values)
    }

    fn get_inputs_by_family(&self, pattern: &PinRole) -> Result<Vec<DataValue>, String> {
        let graph = self.graph.lock().unwrap();
        let all_pins = graph.get_pin_instances_by_node_id(self.node_id);

        let mut values = Vec::new();

        for pin in all_pins {
            if !pin.is_input() {
                continue;
            }
            if pin.definition.role.matches_family(pattern) {
                values.push(graph.get_pin_data_value_by_pin_id(pin.id)?);
            }
        }

        if values.is_empty() {
            return Err(format!("No input pins matching family {:?} found", pattern));
        }

        Ok(values)
    }

    fn emit_output_by_role(&mut self, role: &PinRole, value: DataValue) -> Result<(), String> {
        let mut graph = self.graph.lock().unwrap();
        let pin = graph
            .get_pin_instance_by_pin_role(self.node_id, role)
            .ok_or_else(|| format!("Output pin with role {:?} not found", role))?;

        if pin.is_input() {
            return Err(format!("Pin {:?} is not an output", role));
        }

        graph.set_pin_current_value(pin.id, value);
        
        Ok(())
    }

    fn emit_outputs_by_role(
        &mut self,
        role: &PinRole,
        values: Vec<DataValue>,
    ) -> Result<(), String> {
        let mut graph = self.graph.lock().unwrap();
        let pins = graph.get_pin_instances_by_pin_role(self.node_id, role);

        let output_pins: Vec<PinId> = pins
            .iter()
            .filter(|p| !p.is_input())
            .map(|p| p.id)
            .collect();

        if output_pins.len() != values.len() {
            return Err(format!(
                "Value count mismatch: {} pins, {} values",
                output_pins.len(),
                values.len()
            ));
        }

        for (pin_id, value) in output_pins.into_iter().zip(values) {
            graph.set_pin_current_value(pin_id, value);
        }

        Ok(())
    }

    fn is_input_connected(&self, role: &PinRole) -> bool {
        let graph = self.graph.lock().unwrap();
        graph
            .get_pin_instance_by_pin_role(self.node_id, role)
            .and_then(|pin| graph.get_upstream_by_pin_id(pin.id))
            .is_some()
    }

    fn get_bound_type(&self, _type_var_id: TypeVarId) -> Option<DataType> {
        // TODO: 需要在 GraphRuntime 中实现 get_bound_type
        None
    }

    fn get_pin_type_by_role(&self, role: &PinRole) -> Result<DataType, String> {
        let graph = self.graph.lock().unwrap();
        let pin = graph
            .get_pin_instance_by_pin_role(self.node_id, role)
            .ok_or_else(|| format!("Pin with role {:?} not found", role))?;

        graph.get_pin_data_type_by_pin_role(pin.id)
            .ok_or_else(|| format!("Pin {:?} has no resolved type", role))
    }

    // ====================================================================
    // 节点实例参数
    // ====================================================================

    fn get_instance_params(&self) -> NodeInstanceParams {
        let graph = self.graph.lock().unwrap();
        graph.get_node_instance_params(self.node_id)
    }

    // ====================================================================
    // 数据缓存操作
    // ====================================================================

    fn get_dataframe(&mut self, id: &str) -> Result<Arc<DataFrame>, String> {
        let mut graph = self.graph.lock().unwrap();
        graph.get_dataframe(id)
    }

    fn put_dataframe(&mut self, df: DataFrame) -> Result<String, String> {
        let mut graph = self.graph.lock().unwrap();
        Ok(graph.put_dataframe(df))
    }

    fn get_series(&self, id: &str) -> Result<Series, String> {
        let graph = self.graph.lock().unwrap();
        graph.get_series(id)
    }

    fn put_series(&mut self, s: Series) -> Result<String, String> {
        let mut graph = self.graph.lock().unwrap();
        Ok(graph.put_series(s))
    }

    fn get_variable_value(&self, variable_id: &str) -> Result<DataValue, String> {
        let graph = self.graph.lock().unwrap();
        graph.get_variable_value(variable_id)
    }

    fn set_variable_value(&mut self, variable_id: &str, value: DataValue) -> Result<(), String> {
        let graph = self.graph.lock().unwrap();
        graph.set_variable_value(variable_id, value)
    }

    // ====================================================================
    // 通用句柄存储
    // ====================================================================

    fn put_handle(&mut self, value: Box<dyn Any + Send + Sync>) -> String {
        let mut graph = self.graph.lock().unwrap();
        graph.put_handle_boxed(value)
    }

    fn get_handle(&self, id: &str) -> Result<Arc<dyn Any + Send + Sync>, String> {
        let graph = self.graph.lock().unwrap();
        graph.get_handle(id)
            .ok_or_else(|| format!("Handle '{}' not found", id))
    }

    // ====================================================================
    // 日志
    // ====================================================================

    fn open_window(&mut self, window_type: String, data: String) {
        self.window_actions.push(WindowAction {
            window_type,
            data,
        });
    }

    fn log(&mut self, message: String) {
        self.logs.push(message);
    }

    fn error(&mut self, message: String) {
        self.logs.push(format!("ERROR: {}", message));
    }
}
