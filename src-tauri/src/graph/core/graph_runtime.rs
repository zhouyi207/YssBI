use crate::execution::ExecutionDataStore;
use crate::graph::NodeDefinition;
use crate::graph::node::NodeInstanceParams;
use crate::graph::value::{DataType, DataValue};
use crate::graph::{GraphInstance, NodeId, PinId, PinInstance, PinRole, PinRuntimeState};
use crate::project::{ProjectData, ProjectStore};
use crate::tabular::is_variable_handle;
use crate::variable::{VariableId, VariableScope};
use polars::prelude::{DataFrame, Series};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub struct GraphRuntime {
    graph_instance: Arc<GraphInstance>,

    // 运行期 pin 状态（单次执行内有效，每次 run 前清空）
    pins_runtime_state: HashMap<PinId, PinRuntimeState>,

    /// 控制流循环计数（For / While），每次执行 run 前清空
    loop_counters: HashMap<NodeId, i64>,

    // 执行期数据缓存（中间 DataFrame / Series）
    data_store: ExecutionDataStore,

    // 项目数据引用（变量、数据库元数据）
    project_data: Arc<RwLock<ProjectData>>,

    // 项目运行时存储引用（原始 DataFrame 实例）
    project_store: Arc<RwLock<ProjectStore>>,
}

impl GraphRuntime {
    pub fn new(
        graph_instance: Arc<GraphInstance>,
        project_data: Arc<RwLock<ProjectData>>,
        project_store: Arc<RwLock<ProjectStore>>,
    ) -> Self {
        Self {
            graph_instance,
            pins_runtime_state: HashMap::new(),
            loop_counters: HashMap::new(),
            data_store: ExecutionDataStore::new(),
            project_data,
            project_store,
        }
    }

    /// 简化构造（用于测试等不需要项目数据的场景）
    pub fn new_standalone(graph_instance: Arc<GraphInstance>) -> Self {
        Self::new(
            graph_instance,
            Arc::new(RwLock::new(ProjectData::default())),
            Arc::new(RwLock::new(ProjectStore::default())),
        )
    }

    pub fn graph_id(&self) -> crate::graph::GraphId {
        self.graph_instance.id
    }

    pub fn set_pin_current_value(&mut self, pin_id: PinId, value: DataValue) {
        let pin_instance = self.get_pin_instance_by_pin_id(pin_id).unwrap();
        let pin_runtime_state = self.pins_runtime_state.get_mut(&pin_id);
        if let Some(pin_runtime_state) = pin_runtime_state {
            pin_runtime_state.current_value = Some(value);
        } else {
            let pin_runtime_state =
                PinRuntimeState::from_instance(pin_instance).with_current_value(Some(value));
            self.pins_runtime_state.insert(pin_id, pin_runtime_state);
        }
    }

    pub fn get_pin_instance_by_pin_id(&self, pin_id: PinId) -> Option<PinInstance> {
        self.graph_instance.get_pin_instance_by_pin_id(pin_id)
    }

    pub fn get_pin_instance_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Option<PinInstance> {
        self.graph_instance
            .get_pin_instance_by_pin_role(node_id, role)
    }

    pub fn get_pin_instances_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Vec<PinInstance> {
        self.graph_instance
            .get_pin_instances_by_pin_role(node_id, role)
    }

    pub fn get_pin_instances_by_node_id(&self, node_id: NodeId) -> Vec<PinInstance> {
        self.graph_instance.get_pin_instances_by_node_id(node_id)
    }

    pub fn get_pin_data_value_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Result<DataValue, String> {
        let pin_instance = self
            .graph_instance
            .get_pin_instance_by_pin_role(node_id, role)
            .ok_or_else(|| format!("Pin with role {:?} not found on node {:?}", role, node_id))?;

        if !pin_instance.is_data() {
            return Err(format!("Pin {:?} is not a data pin", pin_instance.id));
        }

        self.resolve_pin_value(pin_instance.id)
            .ok_or_else(|| format!("No value available for pin {:?}", pin_instance.id))
    }

    pub fn get_pin_data_value_by_pin_id(&self, pin_id: PinId) -> Result<DataValue, String> {
        self.resolve_pin_value(pin_id)
            .ok_or_else(|| format!("No value available for pin {:?}", pin_id))
    }

    pub fn get_pin_datas_value_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Result<Vec<DataValue>, String> {
        let pin_instances = self
            .graph_instance
            .get_pin_instances_by_pin_role(node_id, role);

        let mut user_values = vec![];
        for pin_instance in pin_instances {
            if !pin_instance.is_data() {
                return Err(format!("Pin {:?} is not a data pin", pin_instance.id));
            }
            let id = pin_instance.id;
            let value = self
                .resolve_pin_value(id)
                .ok_or_else(|| format!("No value available for pin {:?}", id))?;
            user_values.push(value);
        }

        Ok(user_values)
    }

    pub fn get_pin_data_type_by_pin_role(&self, pin_id: PinId) -> Option<DataType> {
        self.graph_instance.get_pin_data_type_by_pin_id(pin_id)
    }

    /// 从 registry 按 node_type 获取完整定义（含 flow_processor、data_evaluator 等）。
    /// 反序列化后的节点 definition 会丢失 #[serde(skip)] 字段，必须从 registry 补全。
    pub fn get_node_definition_by_node_id(&self, node_id: NodeId) -> Arc<NodeDefinition> {
        let node_instance = self
            .graph_instance
            .get_node_instance_by_node_id(node_id)
            .unwrap();
        let node_type = &node_instance.definition.node_type;
        self.graph_instance
            .registry()
            .get(node_type)
            .unwrap_or_else(|| node_instance.definition)
    }

    pub fn get_node_pins(&self, node_id: NodeId) -> Vec<PinInstance> {
        self.graph_instance.get_pin_instances_by_node_id(node_id)
    }

    pub fn get_downstream_by_pin_id(&self, pin_id: PinId) -> Vec<PinId> {
        self.graph_instance.get_downstream_by_pin_id(pin_id)
    }

    pub fn get_upstream_by_pin_id(&self, pin_id: PinId) -> Option<PinId> {
        self.graph_instance.get_upstream_by_pin_id(pin_id)
    }

    pub fn get_node_id_by_pin_id(&self, pin_id: PinId) -> Option<NodeId> {
        self.graph_instance.get_node_id_by_pin_id(pin_id)
    }

    /// 检查 pin 是否已有由节点执行写入的运行时值（用于执行器缓存判断）。
    /// 仅检查 pins_runtime_state.current_value，不包含 resolve 链中的 default/user 值，
    /// 避免 OneOf 等类型的 default_value（如 Float64）被误判为“已求值”而跳过节点执行。
    pub fn pin_has_executed_value(&self, pin_id: PinId) -> bool {
        self.pins_runtime_state
            .get(&pin_id)
            .and_then(|s| s.current_value.as_ref())
            .map(|v| !matches!(v, DataValue::Null))
            .unwrap_or(false)
    }

    /// 按优先级解析 pin 的值：
    /// 1. 上游连接值（如果有连接且上游有值）
    /// 2. 运行时值（current_value）
    /// 3. 用户值（user_value）
    /// 4. 默认值（default_value）
    ///
    /// 返回前，如果 pin 有已推断的具体类型，自动将值强制转换以保证类型一致。
    pub fn resolve_pin_value(&self, pin_id: PinId) -> Option<DataValue> {
        let pin_instance = self.get_pin_instance_by_pin_id(pin_id)?;

        // 1. 检查上游连接值（最高优先级）
        let raw_value = if let Some(upstream_pin_id) = self.get_upstream_by_pin_id(pin_id) {
            self.resolve_pin_value(upstream_pin_id)
        } else {
            None
        };

        // 2. 检查运行时值
        let raw_value = raw_value.or_else(|| {
            self.pins_runtime_state
                .get(&pin_id)
                .and_then(|s| s.current_value.clone())
        });

        // 3. 检查用户值
        let raw_value = raw_value.or_else(|| pin_instance.user_value.clone());

        // 4. 检查 pin 定义的自定义默认值
        let raw_value = raw_value.or_else(|| pin_instance.definition.default_value.clone());

        // 5. 检查类型默认值
        let raw_value = raw_value.or_else(|| {
            if let Some(pin_data_type_def) = &pin_instance.definition.data_type {
                if let crate::graph::pin::PinDataTypeDefinition::Concrete(data_type) =
                    pin_data_type_def
                {
                    return Some(data_type.default_value());
                }
            }
            None
        });

        // 5. 不进行隐式类型转换，类型转换需使用 convert 节点
        raw_value
    }

    // ========================================================================
    // 节点实例参数
    // ========================================================================

    pub fn get_node_instance_params(&self, node_id: NodeId) -> NodeInstanceParams {
        self.graph_instance
            .get_node_instance_by_node_id(node_id)
            .map(|n| n.instance_params.clone())
            .unwrap_or_default()
    }

    // ========================================================================
    // 控制流循环计数
    // ========================================================================

    pub fn get_loop_counter(&self, node_id: NodeId) -> i64 {
        self.loop_counters.get(&node_id).copied().unwrap_or(0)
    }

    pub fn set_loop_counter(&mut self, node_id: NodeId, value: i64) {
        self.loop_counters.insert(node_id, value);
    }

    pub fn reset_loop_counter(&mut self, node_id: NodeId) {
        self.loop_counters.remove(&node_id);
    }

    pub fn clear_loop_counters(&mut self) {
        self.loop_counters.clear();
    }

    /// 每次图执行开始前重置运行期状态（与 `ExecutionDataStore` 生命周期对齐）。
    pub fn reset_execution_state(&mut self) {
        self.pins_runtime_state.clear();
        self.loop_counters.clear();
        self.data_store.clear();
    }

    // ========================================================================
    // 执行期数据缓存操作
    // ========================================================================

    /// 获取 DataFrame：先查执行缓存，再查变量 tabular 缓存与 ProjectStore 原始数据。
    /// 需要整表的节点（Filter 等）仍走此路径；按列分析应优先 `load_database_data_series`。
    pub fn get_dataframe(&mut self, id: &str) -> Result<Arc<DataFrame>, String> {
        if let Some(df) = self.data_store.get_dataframe(id) {
            return Ok(df);
        }

        if is_variable_handle(id) {
            let store = self.project_store.read().map_err(|e| e.to_string())?;
            if let Some(entry) = store.variable_tabular.get(id) {
                let arc_df = entry.dataframe.clone();
                drop(store);
                self.data_store
                    .put_dataframe_with_id(id.to_string(), arc_df.clone());
                return Ok(arc_df);
            }
            return Err(format!("Variable tabular '{id}' not found"));
        }

        let mut store = self.project_store.write().map_err(|e| e.to_string())?;
        if let Some(db_instance) = store.databases.get_mut(id) {
            if let crate::database::DatabaseState::DuckDb { row_count, .. } = &db_instance.state {
                if *row_count > crate::database::MAX_GET_DATAFRAME_ROWS {
                    return Err(format!(
                        "Dataset '{id}' has {row_count} rows; exceeds in-memory graph limit ({}). \
                         Use column-scoped nodes or filter in DuckDB first.",
                        crate::database::MAX_GET_DATAFRAME_ROWS
                    ));
                }
            }
            let df = db_instance
                .ensure_loaded()
                .map_err(|e| format!("Failed to load database '{id}': {e}"))?;
            let arc_df = Arc::new(df.clone());
            drop(store);
            self.data_store
                .put_dataframe_with_id(id.to_string(), arc_df.clone());
            return Ok(arc_df);
        }

        Err(format!("DataFrame '{id}' not found"))
    }

    fn data_series_cache_key(db_id: &str, column: &str) -> String {
        format!("{db_id}::{column}")
    }

    /// 列出 tabular 列名；执行期中间 DataFrame 走 schema，不整表加载。
    pub fn list_database_columns(&mut self, db_id: &str) -> Result<Vec<String>, String> {
        if let Some(df) = self.data_store.get_dataframe(db_id) {
            return Ok(df
                .schema()
                .iter_names()
                .map(|name| name.to_string())
                .collect());
        }

        if is_variable_handle(db_id) {
            let store = self.project_store.read().map_err(|e| e.to_string())?;
            if let Some(entry) = store.variable_tabular.get(db_id) {
                return Ok(entry.schema.columns.iter().map(|c| c.name.clone()).collect());
            }
            return Err(format!("Variable tabular '{db_id}' not found"));
        }

        let mut store = self.project_store.write().map_err(|e| e.to_string())?;
        if let Some(db) = store.databases.get_mut(db_id) {
            return db.list_column_names().map_err(|e| e.to_string());
        }
        drop(store);

        let df = self.get_dataframe(db_id)?;
        Ok(df
            .schema()
            .iter_names()
            .map(|name| name.to_string())
            .collect())
    }

    /// 按列加载 Series：DuckDB 列裁剪 → Polars；结果缓存在 `data_store`。
    pub fn load_database_data_series(&mut self, db_id: &str, column: &str) -> Result<Series, String> {
        let cache_key = Self::data_series_cache_key(db_id, column);
        if let Some(series) = self.data_store.get_data_series(&cache_key) {
            return Ok(series.clone());
        }

        if let Some(df) = self.data_store.get_dataframe(db_id) {
            let series = df
                .column(column)
                .map_err(|e| format!("Column '{column}' not found in cached DataFrame: {e}"))?
                .clone()
                .take_materialized_series();
            self.data_store
                .put_data_series_with_id(cache_key, series.clone());
            return Ok(series);
        }

        if is_variable_handle(db_id) {
            let store = self.project_store.read().map_err(|e| e.to_string())?;
            if let Some(entry) = store.variable_tabular.get(db_id) {
                let series = entry
                    .dataframe
                    .column(column)
                    .map_err(|e| format!("Column '{column}' not found in variable tabular: {e}"))?
                    .clone()
                    .take_materialized_series();
                drop(store);
                self.data_store
                    .put_data_series_with_id(cache_key, series.clone());
                return Ok(series);
            }
            return Err(format!("Variable tabular '{db_id}' not found"));
        }

        let mut store = self.project_store.write().map_err(|e| e.to_string())?;
        if let Some(db) = store.databases.get_mut(db_id) {
            let series = db
                .load_column_series(column)
                .map_err(|e| format!("Failed to load column '{column}' from '{db_id}': {e}"))?;
            drop(store);
            self.data_store
                .put_data_series_with_id(cache_key, series.clone());
            return Ok(series);
        }
        drop(store);

        let df = self.get_dataframe(db_id)?;
        let series = df
            .column(column)
            .map_err(|e| format!("Column '{column}' not found: {e}"))?
            .clone()
            .take_materialized_series();
        self.data_store
            .put_data_series_with_id(cache_key, series.clone());
        Ok(series)
    }

    /// 存入中间 DataFrame，返回引用 ID
    pub fn put_dataframe(&mut self, df: DataFrame) -> String {
        self.data_store.put_dataframe(df)
    }

    /// 获取 Series：从执行缓存查找
    pub fn get_data_series(&self, id: &str) -> Result<Series, String> {
        self.data_store
            .get_data_series(id)
            .cloned()
            .ok_or_else(|| format!("DataSeries '{}' not found", id))
    }

    /// 存入中间 Series，返回引用 ID
    pub fn put_data_series(&mut self, s: Series) -> String {
        self.data_store.put_data_series(s)
    }

    /// 存入不透明对象，返回句柄 ID
    pub fn put_handle<T: std::any::Any + Send + Sync + 'static>(&mut self, value: T) -> String {
        self.data_store.put_handle(value)
    }

    /// 存入已装箱的不透明对象
    pub fn put_handle_boxed(&mut self, value: Box<dyn std::any::Any + Send + Sync>) -> String {
        self.data_store.put_handle_boxed(value)
    }

    /// 按 ID 获取句柄（Arc 包装，可安全跨锁传递）
    pub fn get_handle(&self, id: &str) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
        self.data_store.get_handle(id)
    }

    // ========================================================================
    // 变量操作
    // ========================================================================

    /// 读取变量值（仅当前图可见的全局变量与本图局部变量）。
    /// DataFrame / DataSeries 变量返回稳定 handle `var:{id}`，数据由 TabularCatalog 解析。
    pub fn get_variable_value(&mut self, variable_id: &str) -> Result<DataValue, String> {
        let var_id = Self::parse_variable_id(variable_id)?;
        let data = self.project_data.read().map_err(|e| e.to_string())?;
        let var = data
            .variables
            .get(&var_id)
            .ok_or_else(|| format!("Variable '{}' not found", variable_id))?;
        if !Self::variable_visible_in_graph(&var.scope, &self.graph_instance.id) {
            return Err(format!(
                "Variable '{}' is not visible in graph '{}'",
                variable_id, self.graph_instance.id
            ));
        }

        Ok(var.data_value.clone())
    }

    /// 写入变量值（仅当前图可见的全局变量与本图局部变量）
    pub fn set_variable_value(&self, variable_id: &str, value: DataValue) -> Result<(), String> {
        let var_id = Self::parse_variable_id(variable_id)?;
        let mut data = self.project_data.write().map_err(|e| e.to_string())?;
        let scope = data
            .variables
            .get(&var_id)
            .map(|v| v.scope.clone())
            .ok_or_else(|| format!("Variable '{}' not found", variable_id))?;
        if !Self::variable_visible_in_graph(&scope, &self.graph_instance.id) {
            return Err(format!(
                "Variable '{}' is not visible in graph '{}'",
                variable_id, self.graph_instance.id
            ));
        }
        let var = data
            .variables
            .get_mut(&var_id)
            .ok_or_else(|| format!("Variable '{}' not found", variable_id))?;
        var.data_value = value;
        Ok(())
    }

    fn variable_visible_in_graph(scope: &VariableScope, graph_id: &crate::graph::GraphId) -> bool {
        match scope {
            VariableScope::Global => true,
            VariableScope::Event { event_id } => event_id == &graph_id.to_string(),
            VariableScope::Function { function_id } => function_id == &graph_id.to_string(),
        }
    }

    fn parse_variable_id(id: &str) -> Result<VariableId, String> {
        let uuid =
            Uuid::parse_str(id).map_err(|e| format!("Invalid variable ID '{}': {}", id, e))?;
        Ok(VariableId::from(uuid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::pin::DataRole;
    use crate::graph::value::{DataType, DataValue};
    use crate::project::{ProjectState, ProjectStore};
    use crate::variable::VariableScope;
    use std::sync::{Arc, RwLock};

    #[test]
    fn get_variable_value_allows_globals_and_own_graph_locals() {
        let state = ProjectState::new();
        let event = state.add_event("Runtime Event");
        let global = state.add_variable(
            "global",
            DataType::Int64,
            DataValue::Int64(1),
            "",
            VariableScope::Global,
            vec![],
        );
        let local = state.add_variable(
            "local",
            DataType::Int64,
            DataValue::Int64(2),
            "",
            VariableScope::Event {
                event_id: event.id.to_string(),
            },
            vec![],
        );
        let graph = state.get_graph(&event.id).unwrap();
        let mut runtime = GraphRuntime::new(
            Arc::new(graph.clone()),
            Arc::new(RwLock::new(state.get_data())),
            Arc::new(RwLock::new(ProjectStore::default())),
        );

        assert_eq!(
            runtime.get_variable_value(&global.id.to_string()).unwrap(),
            DataValue::Int64(1)
        );
        assert_eq!(
            runtime.get_variable_value(&local.id.to_string()).unwrap(),
            DataValue::Int64(2)
        );
    }

    #[test]
    fn get_variable_value_materializes_dataframe_literal_json() {
        let state = ProjectState::new();
        let event = state.add_event("Literal Event");
        let var = state.add_variable(
            "df_var",
            DataType::DataFrame,
            DataValue::DataFrame(r#"{"a":[1,2]}"#.to_string()),
            "",
            VariableScope::Event {
                event_id: event.id.to_string(),
            },
            vec![],
        );
        let graph = state.get_graph(&event.id).unwrap();
        let mut runtime = GraphRuntime::new(
            Arc::new(graph.clone()),
            Arc::new(RwLock::new(state.get_data())),
            Arc::clone(&state.project_store),
        );

        let resolved = runtime.get_variable_value(&var.id.to_string()).unwrap();
        match resolved {
            DataValue::DataFrame(id) => {
                assert!(id.starts_with("var:"));
                let df = runtime.get_dataframe(&id).unwrap();
                assert_eq!(df.height(), 2);
            }
            other => panic!("expected DataFrame ref, got {other:?}"),
        }
    }

    #[test]
    fn get_variable_value_rejects_other_graph_local_variable() {
        let state = ProjectState::new();
        let event_a = state.add_event("Graph A");
        let event_b = state.add_event("Graph B");
        let local = state.add_variable(
            "foreign local",
            DataType::Int64,
            DataValue::Int64(9),
            "",
            VariableScope::Event {
                event_id: event_a.id.to_string(),
            },
            vec![],
        );
        let graph_b = state.get_graph(&event_b.id).unwrap();
        let mut runtime = GraphRuntime::new(
            Arc::new(graph_b.clone()),
            Arc::new(RwLock::new(state.get_data())),
            Arc::new(RwLock::new(ProjectStore::default())),
        );

        let err = runtime
            .get_variable_value(&local.id.to_string())
            .unwrap_err();
        assert!(err.contains("not visible"));
    }

    #[test]
    fn reset_execution_state_clears_pins_and_loop_counters() {
        let registry = Arc::new(crate::graph::register::NodeRegistry::new());
        crate::graph::register::catalog::register_builtin_nodes(&registry);
        let graph = Arc::new(GraphInstance::new(
            "Reset State",
            crate::graph::GraphKind::Event,
            registry,
        ));
        let const_node = graph
            .create_node("Value:Constants:Int64")
            .expect("create constant");
        let result_pin = graph
            .get_pin_instances_by_node_id(const_node)
            .into_iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("result pin");

        let mut runtime = GraphRuntime::new_standalone(graph);
        runtime.set_pin_current_value(result_pin.id, DataValue::Int64(1));
        runtime.set_loop_counter(const_node, 3);
        assert!(runtime.pin_has_executed_value(result_pin.id));
        assert_eq!(runtime.get_loop_counter(const_node), 3);

        runtime.reset_execution_state();

        assert!(!runtime.pin_has_executed_value(result_pin.id));
        assert_eq!(runtime.get_loop_counter(const_node), 0);
    }
}
