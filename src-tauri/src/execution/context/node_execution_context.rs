use super::NodeExecutionContextTrait;
use crate::execution::{ExecutionEvent, ResultSourceRecord, ResultSourceStore};
use crate::graph::GraphId;
use crate::graph::core::GraphRuntime;
use crate::graph::infer::TypeVarId;
use crate::graph::node::{NodeId, NodeInstanceParams};
use crate::graph::pin::{ExecRole, PinId, PinRole};
use crate::graph::value::{DataType, DataValue};
use polars::prelude::{DataFrame, Series};
use std::any::Any;
use std::sync::{Arc, Mutex};

/// 窗口打开请求
pub struct WindowAction {
    pub window_type: String,
    pub data: Option<String>,
    pub source_record: Option<ResultSourceRecord>,
    pub existing_source_id: Option<String>,
}

/// 具体的执行上下文实现
pub struct NodeExecutionContext {
    pub node_id: NodeId,
    pub graph: Arc<Mutex<GraphRuntime>>,
    pub logs: Vec<String>,
    pub window_actions: Vec<WindowAction>,
    pub pin_result_events: Vec<ExecutionEvent>,
    result_source_store: ResultSourceStore,
    run_id: String,
}

impl NodeExecutionContext {
    pub fn new(graph: Arc<Mutex<GraphRuntime>>, node_id: NodeId) -> Self {
        Self::with_result_sources(graph, node_id, ResultSourceStore::new(), "test".to_string())
    }

    pub fn with_result_sources(
        graph: Arc<Mutex<GraphRuntime>>,
        node_id: NodeId,
        result_source_store: ResultSourceStore,
        run_id: String,
    ) -> Self {
        Self {
            node_id,
            graph,
            logs: Vec::new(),
            window_actions: Vec::new(),
            pin_result_events: Vec::new(),
            result_source_store,
            run_id,
        }
    }

    fn register_output_source(
        &mut self,
        graph_id: GraphId,
        pin_id: PinId,
        value: &DataValue,
    ) -> Result<(), String> {
        let source_id = format!("runtime_{}_{}_{}", self.run_id, graph_id, pin_id);
        let record =
            self.build_source_record_for_value(source_id.clone(), "", value, None)?;

        let descriptor = self.result_source_store.insert_runtime_pin_source(
            graph_id.to_string(),
            pin_id.to_string(),
            self.run_id.clone(),
            record,
        );
        self.pin_result_events.push(ExecutionEvent::PinResultReady {
            graph_id: graph_id.to_string(),
            node_id: self.node_id.to_string(),
            pin_id: pin_id.to_string(),
            source_id,
            descriptor,
        });
        Ok(())
    }

    fn build_source_record_for_value(
        &mut self,
        source_id: String,
        title: impl Into<String>,
        value: &DataValue,
        execution_time_ms: Option<u64>,
    ) -> Result<ResultSourceRecord, String> {
        let resolved = match value {
            DataValue::Null => crate::execution::ResolvedSourceValue::Null,
            DataValue::DataFrame(id) => {
                crate::execution::ResolvedSourceValue::DataFrame(self.get_dataframe(id)?)
            }
            DataValue::DataSeries(v) => {
                crate::execution::ResolvedSourceValue::DataSeries(self.get_data_series(&v.id)?)
            }
            DataValue::Struct {
                type_key,
                handle_id,
            } => crate::execution::ResolvedSourceValue::Struct {
                type_key: type_key.clone(),
                handle_id: handle_id.clone(),
                handle: self.get_handle(handle_id).ok(),
            },
            other => crate::execution::ResolvedSourceValue::Value(other.clone()),
        };

        crate::execution::build_source_from_resolved(
            source_id,
            title.into(),
            value,
            resolved,
            execution_time_ms,
        )
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

        Ok(values)
    }

    fn emit_output_by_role(&mut self, role: &PinRole, value: DataValue) -> Result<(), String> {
        let (graph_id, pin_id) = {
            let mut graph = self.graph.lock().unwrap();
            let pin = graph
                .get_pin_instance_by_pin_role(self.node_id, role)
                .ok_or_else(|| format!("Output pin with role {:?} not found", role))?;

            if pin.is_input() {
                return Err(format!("Pin {:?} is not an output", role));
            }

            graph.set_pin_current_value(pin.id, value.clone());
            (graph.graph_id(), pin.id)
        };

        self.register_output_source(graph_id, pin_id, &value)?;

        Ok(())
    }

    fn emit_outputs_by_role(
        &mut self,
        role: &PinRole,
        values: Vec<DataValue>,
    ) -> Result<(), String> {
        let (graph_id, output_pins) = {
            let graph = self.graph.lock().unwrap();
            let pins = graph.get_pin_instances_by_pin_role(self.node_id, role);
            let output_pins: Vec<PinId> = pins
                .iter()
                .filter(|p| !p.is_input())
                .map(|p| p.id)
                .collect();
            (graph.graph_id(), output_pins)
        };

        if output_pins.len() != values.len() {
            return Err(format!(
                "Value count mismatch: {} pins, {} values",
                output_pins.len(),
                values.len()
            ));
        }

        for (pin_id, value) in output_pins.into_iter().zip(values) {
            {
                let mut graph = self.graph.lock().unwrap();
                graph.set_pin_current_value(pin_id, value.clone());
            }
            self.register_output_source(graph_id, pin_id, &value)?;
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

        graph
            .get_pin_data_type_by_pin_role(pin.id)
            .ok_or_else(|| format!("Pin {:?} has no resolved type", role))
    }

    fn get_resolved_value_by_role(&self, role: &PinRole) -> Result<DataValue, String> {
        let graph = self.graph.lock().unwrap();
        let pin = graph
            .get_pin_instance_by_pin_role(self.node_id, role)
            .ok_or_else(|| format!("Pin with role {:?} not found", role))?;

        graph.get_pin_data_value_by_pin_id(pin.id)
    }

    fn get_exec_step_outputs(&self) -> Vec<ExecRole> {
        let graph = self.graph.lock().unwrap();
        let mut steps: Vec<(usize, ExecRole)> = graph
            .get_pin_instances_by_node_id(self.node_id)
            .iter()
            .filter(|pin| pin.is_output() && pin.is_exec())
            .filter_map(|pin| match pin.definition.role {
                PinRole::Exec(ExecRole::Steps(index)) => Some((index, ExecRole::Steps(index))),
                _ => None,
            })
            .collect();
        steps.sort_by_key(|(index, _)| *index);
        steps.into_iter().map(|(_, role)| role).collect()
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

    fn list_database_columns(&mut self, db_id: &str) -> Result<Vec<String>, String> {
        let mut graph = self.graph.lock().unwrap();
        graph.list_database_columns(db_id)
    }

    fn load_database_data_series(&mut self, db_id: &str, column: &str) -> Result<Series, String> {
        let mut graph = self.graph.lock().unwrap();
        graph.load_database_data_series(db_id, column)
    }

    fn put_dataframe(&mut self, df: DataFrame) -> Result<String, String> {
        let mut graph = self.graph.lock().unwrap();
        Ok(graph.put_dataframe(df))
    }

    fn get_data_series(&self, id: &str) -> Result<Series, String> {
        let graph = self.graph.lock().unwrap();
        graph.get_data_series(id)
    }

    fn put_data_series(&mut self, s: Series) -> Result<String, String> {
        let mut graph = self.graph.lock().unwrap();
        Ok(graph.put_data_series(s))
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
        graph
            .get_handle(id)
            .ok_or_else(|| format!("Handle '{}' not found", id))
    }

    // ====================================================================
    // 日志
    // ====================================================================

    fn open_window(&mut self, window_type: String, data: String) {
        self.window_actions.push(WindowAction {
            window_type,
            data: Some(data),
            source_record: None,
            existing_source_id: None,
        });
    }

    fn open_result_source_window(&mut self, window_type: String, record: ResultSourceRecord) {
        self.window_actions.push(WindowAction {
            window_type,
            data: None,
            source_record: Some(record),
            existing_source_id: None,
        });
    }

    fn open_existing_source_window(&mut self, window_type: String, source_id: String) {
        self.window_actions.push(WindowAction {
            window_type,
            data: None,
            source_record: None,
            existing_source_id: Some(source_id),
        });
    }

    fn ensure_view_source_for_input(&mut self, role: &PinRole) -> Result<String, String> {
        if let Some(source_id) = self.get_input_source_id_by_role(role) {
            return Ok(source_id);
        }

        let value = self.get_input_by_role(role)?;
        let source_id = format!("window_{}", uuid::Uuid::new_v4().simple());
        let record = self.build_source_record_for_value(source_id.clone(), "", &value, None)?;
        self.result_source_store.insert_window_source(record);
        Ok(source_id)
    }

    fn get_input_source_id_by_role(&self, role: &PinRole) -> Option<String> {
        let (graph_id, upstream_pin_id) = {
            let graph = self.graph.lock().ok()?;
            let input = graph.get_pin_instance_by_pin_role(self.node_id, role)?;
            let upstream = graph.get_upstream_by_pin_id(input.id)?;
            (graph.graph_id().to_string(), upstream.to_string())
        };
        self.result_source_store
            .get_pin_descriptor(&graph_id, &upstream_pin_id)
            .map(|descriptor| descriptor.source_id)
    }

    fn log(&mut self, message: String) {
        self.logs.push(message);
    }

    fn error(&mut self, message: String) {
        self.logs.push(format!("ERROR: {}", message));
    }
}
