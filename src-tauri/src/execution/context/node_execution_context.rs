use super::NodeExecutionContextTrait;
use crate::execution::{
    Executor, ExecutionEvent, NoopEmitter, PlotChart, Presentation, ReportKind, ResultSourceRecord,
    ResultSourceStore,
};
use crate::graph::core::GraphRuntime;
use crate::graph::infer::TypeVarId;
use crate::graph::node::{NodeId, NodeInstanceParams};
use crate::graph::pin::{DataRole, ExecRole, PinId, PinRole};
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
        let call_node_id = self.node_id;

        // 目标函数 id（来自 Call 节点 SubGraph 参数）
        let sub_graph_path = {
            let rt = self.graph.lock().unwrap();
            rt.get_node_instance_params(call_node_id)
                .sub_graph_path()
                .map(|s| s.to_string())
        }
        .ok_or_else(|| "Call Function: subGraphPath 未设置".to_string())?;

        let function_path = crate::project::GraphResourcePath::new(sub_graph_path.clone())
            .map_err(|e| format!("Call Function: 无效 subGraphPath '{}': {}", sub_graph_path, e))?;

        // 取项目引用 + 目标函数图（clone 共享 data_state Arc，执行期只读，安全）
        let (project_data, project_store, function_graph) = {
            let rt = self.graph.lock().unwrap();
            let pd = rt.project_data();
            let ps = rt.project_store();
            let fg = pd
                .read()
                .unwrap()
                .graphs
                .get(&function_path)
                .cloned()
                .ok_or_else(|| format!("Call Function: 目标函数图 {} 未加载", sub_graph_path))?;
            (pd, ps, fg)
        };

        let (entry_id, return_id) = function_graph.find_function_shell_nodes();
        // 以 Call 节点实例上的 exec 引脚为准（签名投影结果），避免与目标图签名二次判定分叉。
        let has_exec_input = {
            let rt = self.graph.lock().unwrap();
            rt.node_has_exec_pins(call_node_id)
        };
        let function_inputs = function_graph.function_inputs.clone();
        let function_outputs = function_graph.function_outputs.clone();
        let return_id =
            return_id.ok_or_else(|| "Call Function: 目标函数缺少 Return 节点".to_string())?;

        // 读取本 Call 节点数据输入（按签名 id 的 Data(Custom) role；跳过 exec 项）。
        let mut inputs: Vec<(String, DataValue)> = Vec::new();
        for sig in &function_inputs {
            if sig.is_exec() {
                continue;
            }
            let role = PinRole::Data(DataRole::Custom(sig.id.clone()));
            if let Ok(value) = self.get_input_by_role(&role) {
                inputs.push((sig.id.clone(), value));
            }
        }

        // 递归深度保护（同一执行线程内）
        let _depth = CallDepthGuard::enter()?;

        // 构建嵌套 runtime，预置 Entry 输出（入参）值。
        //
        // 每次调用新建独立 runtime 是刻意为之，且开销很低：
        // - `GraphInstance` 是浅克隆（`data_state` 为 `Arc<RwLock<_>>`），不复制节点 / pin 数据。
        // - `GraphRuntime::new` 只分配空 HashMap，执行期状态（`pins_runtime_state` / `loop_counters`）
        //   独立于共享的 `data_state`。
        // 因此不缓存复用：缓存反而会在递归 / 多 Call 指向同一函数时共享可变执行状态而互相污染，
        // 得不偿失。
        let mut runtime = GraphRuntime::new(Arc::new(function_graph), project_data, project_store);
        runtime.reset_execution_state();
        if let Some(entry) = entry_id {
            for (sig_id, value) in inputs {
                let role = PinRole::Data(DataRole::Custom(sig_id));
                if let Some(pin) = runtime.get_pin_instance_by_pin_role(entry, &role) {
                    runtime.set_pin_current_value(pin.id, value);
                }
            }
        }

        let runtime = Arc::new(Mutex::new(runtime));
        let mut executor = Executor::new(
            Arc::clone(&runtime),
            NoopEmitter,
            self.result_source_store.clone(),
        );

        // 运行函数体：无 exec 入参时按数据拉取 Return；否则从 Entry 走控制流子程序。
        if !has_exec_input {
            executor.evaluate_data_target(return_id)?;
        } else {
            let entry =
                entry_id.ok_or_else(|| "Call Function: 目标函数缺少 Entry 节点".to_string())?;
            executor.run_subroutine(entry)?;
        }
        self.logs.extend(executor.logs().iter().cloned());

        // 读取 Return 数据输入值（跳过 exec 项）。
        let mut outputs: Vec<(String, DataValue)> = Vec::new();
        {
            let rt = runtime.lock().unwrap();
            for sig in &function_outputs {
                if sig.is_exec() {
                    continue;
                }
                let role = PinRole::Data(DataRole::Custom(sig.id.clone()));
                if let Ok(value) = rt.get_pin_data_value_by_pin_role(return_id, &role) {
                    outputs.push((sig.id.clone(), value));
                }
            }
        }

        // 写回本 Call 节点输出（若对应 pin 存在）
        for (sig_id, value) in outputs {
            let role = PinRole::Data(DataRole::Custom(sig_id));
            let has_pin = {
                let rt = self.graph.lock().unwrap();
                rt.get_pin_instance_by_pin_role(call_node_id, &role).is_some()
            };
            if has_pin {
                self.emit_output_by_role(&role, value)?;
            }
        }

        Ok(())
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

/// 函数调用最大嵌套深度（防止直接 / 间接递归导致栈溢出）。
const MAX_CALL_DEPTH: usize = 64;

thread_local! {
    static CALL_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// RAII：进入函数调用时 +1，离开（含出错）时 -1。
struct CallDepthGuard;

impl CallDepthGuard {
    fn enter() -> Result<Self, String> {
        CALL_DEPTH.with(|d| {
            if d.get() >= MAX_CALL_DEPTH {
                return Err(format!(
                    "Call Function: 调用嵌套超过上限 {}（可能存在递归调用）",
                    MAX_CALL_DEPTH
                ));
            }
            d.set(d.get() + 1);
            Ok(CallDepthGuard)
        })
    }
}

impl Drop for CallDepthGuard {
    fn drop(&mut self) {
        CALL_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}
