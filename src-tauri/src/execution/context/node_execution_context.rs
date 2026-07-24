use super::NodeExecutionContextTrait;
use crate::execution::{
    ExecutionEvent, PlotChart, Presentation, ReportKind, ResultSourceRecord, ResultSourceStore,
};
use crate::graph::core::GraphRuntime;

use crate::graph::node::{NodeId, NodeInstanceParams};
use crate::graph::pin::{ExecRole, PinId, PinRole};
use crate::graph::value::{DataType, DataValue};
use polars::prelude::{DataFrame, Series};
use std::any::Any;
use std::sync::{Arc, Mutex};

/// Result source publish request collected during node execution.
pub enum SourceAction {
    PublishJson {
        presentation: Presentation,
        data: String,
    },
    PublishRecord(ResultSourceRecord),
}

/// 具体的执行上下文实现
pub struct NodeExecutionContext {
    pub node_id: NodeId,
    pub graph: Arc<Mutex<GraphRuntime>>,
    pub logs: Vec<String>,
    pub source_actions: Vec<SourceAction>,
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
            source_actions: Vec::new(),
            pin_result_events: Vec::new(),
            result_source_store,
            run_id,
        }
    }

    fn register_output_source(
        &mut self,
        graph_path: &str,
        pin_id: PinId,
        value: &DataValue,
    ) -> Result<(), String> {
        let source_id = format!("runtime_{}_{}_{}", self.run_id, graph_path, pin_id);
        let record = self.build_source_record_for_value(source_id.clone(), "", value, None)?;

        let descriptor = self.result_source_store.insert_runtime_pin_source(
            graph_path.to_string(),
            pin_id.to_string(),
            self.run_id.clone(),
            record,
        );
        self.pin_result_events.push(ExecutionEvent::PinResultReady {
            graph_path: graph_path.to_string(),
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
        let (graph_path, pin_id) = {
            let mut graph = self.graph.lock().unwrap();
            let pin = graph
                .get_pin_instance_by_pin_role(self.node_id, role)
                .ok_or_else(|| format!("Output pin with role {:?} not found", role))?;

            if pin.is_input() {
                return Err(format!("Pin {:?} is not an output", role));
            }

            graph.set_pin_current_value(pin.id, value.clone());
            (graph.graph_path(), pin.id)
        };

        self.register_output_source(graph_path.as_str(), pin_id, &value)?;

        Ok(())
    }

    fn emit_outputs_by_role(
        &mut self,
        role: &PinRole,
        values: Vec<DataValue>,
    ) -> Result<(), String> {
        let (graph_path, output_pins) = {
            let graph = self.graph.lock().unwrap();
            let pins = graph.get_pin_instances_by_pin_role(self.node_id, role);
            let output_pins: Vec<PinId> = pins
                .iter()
                .filter(|p| !p.is_input())
                .map(|p| p.id)
                .collect();
            (graph.graph_path(), output_pins)
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
            self.register_output_source(graph_path.as_str(), pin_id, &value)?;
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

    fn get_exec_output_roles(&self) -> Vec<ExecRole> {
        let graph = self.graph.lock().unwrap();
        graph
            .get_pin_instances_by_node_id(self.node_id)
            .into_iter()
            .filter(|pin| pin.is_output() && pin.is_exec())
            .filter_map(|pin| match pin.definition.role {
                PinRole::Exec(role) => Some(role),
                _ => None,
            })
            .collect()
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

    fn get_exec_case_outputs(&self) -> Vec<ExecRole> {
        let graph = self.graph.lock().unwrap();
        let mut cases: Vec<(usize, ExecRole)> = graph
            .get_pin_instances_by_node_id(self.node_id)
            .iter()
            .filter(|pin| pin.is_output() && pin.is_exec())
            .filter_map(|pin| match pin.definition.role {
                PinRole::Exec(ExecRole::Cases(index)) => Some((index, ExecRole::Cases(index))),
                _ => None,
            })
            .collect();
        cases.sort_by_key(|(index, _)| *index);
        cases.into_iter().map(|(_, role)| role).collect()
    }

    fn get_loop_counter(&self) -> i64 {
        let graph = self.graph.lock().unwrap();
        graph.get_loop_counter(self.node_id)
    }

    fn set_loop_counter(&mut self, value: i64) {
        let mut graph = self.graph.lock().unwrap();
        graph.set_loop_counter(self.node_id, value);
    }

    fn reset_loop_counter(&mut self) {
        let mut graph = self.graph.lock().unwrap();
        graph.reset_loop_counter(self.node_id);
    }

    // ====================================================================
    // 节点实例参数
    // ====================================================================

    fn get_instance_params(&self) -> NodeInstanceParams {
        let graph = self.graph.lock().unwrap();
        graph.get_node_instance_params(self.node_id)
    }

    fn call_subgraph(&mut self) -> Result<(), String> {
        Err("legacy GraphInstance subgraph execution is not a production path".to_string())
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

    fn get_variable_value(&mut self, variable_id: &str) -> Result<DataValue, String> {
        let mut graph = self.graph.lock().unwrap();
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

    fn publish_json(&mut self, presentation: Presentation, data: String) {
        self.source_actions
            .push(SourceAction::PublishJson { presentation, data });
    }

    fn publish_plot(&mut self, chart: PlotChart, data: String) {
        self.publish_json(Presentation::Plot { chart }, data);
    }

    fn publish_report(&mut self, report: ReportKind, data: String) {
        self.publish_json(Presentation::Report { report }, data);
    }

    fn publish_record(&mut self, record: ResultSourceRecord) {
        self.source_actions
            .push(SourceAction::PublishRecord(record));
    }

    fn ensure_view_source_for_input(&mut self, role: &PinRole) -> Result<String, String> {
        let value = match self.get_input_by_role(role) {
            Ok(value) => value,
            Err(_) if !self.is_input_connected(role) => DataValue::Null,
            Err(err) => return Err(err),
        };
        let source_id = format!("window_{}", uuid::Uuid::new_v4().simple());
        let title = crate::execution::default_view_title(&value, None);
        let record = self.build_source_record_for_value(source_id.clone(), title, &value, None)?;
        self.publish_record(record);
        Ok(source_id)
    }

    fn log(&mut self, message: String) {
        self.logs.push(message);
    }

    fn error(&mut self, message: String) {
        self.logs.push(format!("ERROR: {}", message));
    }
}
