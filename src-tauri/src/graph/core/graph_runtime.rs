use crate::execution::ExecutionDataStore;
use crate::graph::node::NodeInstanceParams;
use crate::graph::NodeDefinition;
use crate::graph::{DataType, DataValue, GraphInstance, PinInstance, PinRole};
use crate::graph::{NodeId, NodeRuntimeState, PinId, PinRuntimeState};
use crate::project::{ProjectData, ProjectStore};
use crate::variable::VariableId;
use polars::prelude::{DataFrame, Series};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub struct GraphRuntime {
    graph_instance: Arc<GraphInstance>,

    // 运行期 pin 状态
    pins_runtime_state: HashMap<PinId, PinRuntimeState>,

    // 运行期 node 状态
    nodes_runtime_state: HashMap<NodeId, NodeRuntimeState>,

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
            nodes_runtime_state: HashMap::new(),
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

    pub fn set_pin_current_value(&mut self, pin_id: PinId, value: DataValue) {
        let pin_instance = self.get_pin_instance_by_pin_id(pin_id).unwrap();
        let pin_runtime_state = self.pins_runtime_state.get_mut(&pin_id);
        if let Some(pin_runtime_state) = pin_runtime_state {
            pin_runtime_state.current_value = Some(value);
        } else {
            let pin_runtime_state = PinRuntimeState::from_instance(pin_instance).with_current_value(Some(value));
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

    pub fn get_pin_data_value_by_pin_role(&self, node_id: NodeId, role: &PinRole) -> DataValue {
        let pin_instance = self
            .graph_instance
            .get_pin_instance_by_pin_role(node_id, role)
            .unwrap();
        assert_eq!(pin_instance.is_data(), true);

        // 使用 resolve_pin_value 按优先级获取值
        self.resolve_pin_value(pin_instance.id)
            .unwrap_or_else(|| panic!("No value available for pin {:?}", pin_instance.id))
    }

    pub fn get_pin_data_value_by_pin_id(&self, pin_id: PinId) -> DataValue {
        // 使用 resolve_pin_value 按优先级获取值
        self.resolve_pin_value(pin_id)
            .unwrap_or_else(|| panic!("No value available for pin {:?}", pin_id))
    }

    pub fn get_pin_datas_value_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Vec<DataValue> {
        let pin_instances = self
            .graph_instance
            .get_pin_instances_by_pin_role(node_id, role);

        let mut user_values = vec![];
        for pin_instance in pin_instances {
            assert_eq!(pin_instance.is_data(), true);
            let id = pin_instance.id;
            if let Some(pin_runtime_state) = self.pins_runtime_state.get(&id) {
                user_values.push(pin_runtime_state.current_value.clone().unwrap());
            } else {
                user_values.push(pin_instance.user_value.unwrap());
            }
        }

        user_values
    }

    pub fn get_pin_data_type_by_pin_role(&self, pin_id: PinId) -> Option<DataType> {
        self.graph_instance.get_pin_data_type_by_pin_id(pin_id)
    }

    pub fn get_node_definition_by_node_id(&self, node_id: NodeId) -> Arc<NodeDefinition> {
        let node_instance = self
            .graph_instance
            .get_node_instance_by_node_id(node_id)
            .unwrap();
        node_instance.definition
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

    pub fn get_node_id_by_pin_id(&self, pin_id: PinId) -> NodeId {
        self.graph_instance.get_node_id_by_pin_id(pin_id)
    }

    /// 按优先级解析 pin 的值：
    /// 1. 上游连接值（如果有连接且上游有值）
    /// 2. 运行时值（current_value）
    /// 3. 用户值（user_value）
    /// 4. 默认值（default_value）
    pub fn resolve_pin_value(&self, pin_id: PinId) -> Option<DataValue> {
        let pin_instance = self.get_pin_instance_by_pin_id(pin_id)?;
        
        // 1. 检查上游连接值（最高优先级）
        if let Some(upstream_pin_id) = self.get_upstream_by_pin_id(pin_id) {
            // 递归解析上游 pin 的值（这样可以处理多层连接和常量节点）
            if let Some(upstream_value) = self.resolve_pin_value(upstream_pin_id) {
                return Some(upstream_value);
            }
        }
        
        // 2. 检查运行时值
        if let Some(pin_runtime_state) = self.pins_runtime_state.get(&pin_id) {
            if let Some(current_value) = &pin_runtime_state.current_value {
                return Some(current_value.clone());
            }
        }
        
        // 3. 检查用户值
        if let Some(user_value) = &pin_instance.user_value {
            return Some(user_value.clone());
        }
        
        // 4. 检查默认值
        if let Some(pin_data_type_def) = &pin_instance.definition.data_type {
            if let crate::graph::pin::PinDataTypeDefinition::Concrete(data_type) = pin_data_type_def {
                return Some(data_type.default_value());
            }
        }
        
        None
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
    // 执行期数据缓存操作
    // ========================================================================

    /// 获取 DataFrame：先查执行缓存，再查 ProjectStore 原始数据
    pub fn get_dataframe(&mut self, id: &str) -> Result<Arc<DataFrame>, String> {
        // 1. 先从执行缓存查找
        if let Some(df) = self.data_store.get_dataframe(id) {
            return Ok(df);
        }

        // 2. 再从 ProjectStore 原始数据库加载
        let mut store = self.project_store.write().map_err(|e| e.to_string())?;
        if let Some(db_instance) = store.databases.get_mut(id) {
            let df = db_instance
                .ensure_loaded()
                .map_err(|e| format!("Failed to load database '{}': {}", id, e))?;
            let arc_df = Arc::new(df.clone());
            // 缓存到执行存储中，后续访问不再触发 IO
            self.data_store
                .put_dataframe_with_id(id.to_string(), arc_df.clone());
            return Ok(arc_df);
        }

        Err(format!("DataFrame '{}' not found", id))
    }

    /// 存入中间 DataFrame，返回引用 ID
    pub fn put_dataframe(&mut self, df: DataFrame) -> String {
        self.data_store.put_dataframe(df)
    }

    /// 获取 Series：从执行缓存查找
    pub fn get_series(&self, id: &str) -> Result<Series, String> {
        self.data_store
            .get_series(id)
            .cloned()
            .ok_or_else(|| format!("Series '{}' not found", id))
    }

    /// 存入中间 Series，返回引用 ID
    pub fn put_series(&mut self, s: Series) -> String {
        self.data_store.put_series(s)
    }

    // ========================================================================
    // 变量操作
    // ========================================================================

    /// 读取变量值
    pub fn get_variable_value(&self, variable_id: &str) -> Result<DataValue, String> {
        let var_id = Self::parse_variable_id(variable_id)?;
        let data = self.project_data.read().map_err(|e| e.to_string())?;
        data.variables
            .get(&var_id)
            .map(|v| v.data_value.clone())
            .ok_or_else(|| format!("Variable '{}' not found", variable_id))
    }

    /// 写入变量值
    pub fn set_variable_value(
        &self,
        variable_id: &str,
        value: DataValue,
    ) -> Result<(), String> {
        let var_id = Self::parse_variable_id(variable_id)?;
        let mut data = self.project_data.write().map_err(|e| e.to_string())?;
        let var = data
            .variables
            .get_mut(&var_id)
            .ok_or_else(|| format!("Variable '{}' not found", variable_id))?;
        var.data_value = value;
        Ok(())
    }

    fn parse_variable_id(id: &str) -> Result<VariableId, String> {
        let uuid = Uuid::parse_str(id)
            .map_err(|e| format!("Invalid variable ID '{}': {}", id, e))?;
        Ok(VariableId::from(uuid))
    }
}
