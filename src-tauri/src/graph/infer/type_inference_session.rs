use super::{TypeInferenceContext, TypeVarKey};
use crate::graph::pin::PinDataTypeInference;
use crate::graph::{DataType, GraphInstance, GraphValidationWarning, PinId, TypeVarId};
use std::collections::HashMap;

/// 一次推断会话
pub struct TypeInferenceSession<'g> {
    pub graph: &'g GraphInstance,
    pub ctx: TypeInferenceContext,
}

impl<'g> TypeInferenceSession<'g> {
    /// 创建新的 Session
    pub fn new(graph: &'g GraphInstance) -> Self {
        Self {
            graph,
            ctx: TypeInferenceContext::new(),
        }
    }

    /// 注册图中所有节点/Pin/类型变量
    /// 可以根据需要改成增量注册
    pub fn register_all(&mut self) {
        let data_state = self.graph.data_state.read().unwrap();

        // 1. 先注册所有节点的类型变量 + 兄弟映射
        for node_instance in data_state.nodes.values() {
            let mut sibling_map: HashMap<TypeVarKey, TypeVarId> = HashMap::new();
            for (&type_var_id, type_var_def) in &node_instance.type_var_map {
                let type_var_inference = super::TypeVarInference {
                    id: type_var_id,
                    constraints: type_var_def.constraints.clone(),
                    bound: type_var_def.bound.clone(),
                };
                self.ctx.register_type_var(type_var_inference);
                sibling_map.insert(type_var_def.id.clone(), type_var_id);
            }
            if sibling_map.len() > 1 {
                self.ctx.register_sibling_map(sibling_map);
            }
        }

        // 2. 然后注册所有 Pin（只注册有类型描述的 Data Pin）
        for pin_instance in data_state.pins.values() {
            self.ctx.register_pin_type(pin_instance.clone());
        }

        // 3. 用 data_state.pin_types 中已有的具体类型覆盖定义中的 Any
        //    （如 variable 节点创建时根据 instance_params 设定的 pin 类型）
        for (&pin_id, resolved) in data_state.pin_types.iter() {
            if *resolved == DataType::Any {
                continue;
            }
            if let Some(existing) = self.ctx.pin_types.get(&pin_id) {
                if matches!(existing, PinDataTypeInference::Concrete(DataType::Any)) {
                    self.ctx
                        .pin_types
                        .insert(pin_id, PinDataTypeInference::Concrete(resolved.clone()));
                }
            }
        }
    }

    /// 推断整张图（全量，逐边 best-effort）
    ///
    /// 单条连接 unify 失败（历史脏边/类型冲突）只记 warn 并跳过，不再 `?` 传播
    /// 而毒化整图——其余连接照常推断与传播。并查集 + 绑定合并对一致图与边序
    /// 无关，故无需排序。`commit` 仍保持严格（绑定冲突属真错）。
    pub fn infer_all(&mut self) -> Result<Vec<GraphValidationWarning>, String> {
        let mut warnings = Vec::new();
        let connections = self
            .graph
            .data_state
            .read()
            .unwrap()
            .connections
            .all_connections();
        for connection in connections {
            if let Err(e) = self
                .ctx
                .infer_connection(connection.from_pin, connection.to_pin)
            {
                warnings.push(GraphValidationWarning {
                    code: "incompatible_connection",
                    from_pin_id: connection.from_pin,
                    to_pin_id: connection.to_pin,
                    message: e.clone(),
                });
                crate::log::log_sys::warn!(
                    "type inference skipped an incompatible connection {:?} -> {:?}: {}",
                    connection.from_pin,
                    connection.to_pin,
                    e
                );
            }
        }
        // Convert 节点：不再将 Input 与 Output 统一，二者独立推断
        // - Input 由上游连接确定
        // - Output 由下游连接确定
        // - 二者均可为不同类型（如 Input=Float, Output=Int），由 convert_to_type 在运行时执行转换
        Ok(warnings)
    }

    /// 提交结果：把临时绑定写回 TypeVarDefinition.bound
    pub fn commit(&mut self) -> Result<(), String> {
        self.ctx.commit()
    }

    /// 提交推断结果到 graph data_state，返回所有被写入的 (PinId, DataType) 列表
    pub fn commit_to_graph(&mut self) -> Result<Vec<(PinId, DataType)>, String> {
        let mut data_state = self.graph.data_state.write().unwrap();
        let mut resolved = Vec::new();

        let pin_ids_with_data: Vec<_> = self
            .ctx
            .pin_types
            .iter()
            .map(|(&pin_id, pin_data)| (pin_id, pin_data.clone()))
            .collect();
        for (pin_id, pin_data) in pin_ids_with_data {
            match self.ctx.resolve_pin_type(pin_id) {
                Ok(concrete_type) => {
                    data_state.pin_types.insert(pin_id, concrete_type.clone());
                    resolved.push((pin_id, concrete_type));
                }
                Err(_) => {
                    // TypeVar pin 没有绑定 — 恢复为 Any（未确定状态）
                    if matches!(pin_data, PinDataTypeInference::TypeVar(_)) {
                        let fallback = DataType::Any;
                        data_state.pin_types.insert(pin_id, fallback.clone());
                        resolved.push((pin_id, fallback));
                    }
                }
            }
        }

        // 写回 TypeVar 绑定：仅保留各节点 type_var_map 中的 TypeVarId
        let live_type_var_ids: std::collections::HashSet<TypeVarId> = data_state
            .nodes
            .values()
            .flat_map(|node| node.type_var_map.keys().copied())
            .collect();

        self.ctx.commit()?;

        data_state
            .type_var_bindings
            .retain(|var_id, _| live_type_var_ids.contains(var_id));

        for &var_id in &live_type_var_ids {
            if let Some(var_def) = self.ctx.type_vars.get(&var_id) {
                if let Some(bound_type) = &var_def.bound {
                    data_state
                        .type_var_bindings
                        .insert(var_id, bound_type.clone());
                } else {
                    data_state.type_var_bindings.remove(&var_id);
                }
            }
        }

        Ok(resolved)
    }

    /// 查询某个 pin 的最终类型
    pub fn resolve_pin(&mut self, pin_id: PinId) -> Result<DataType, String> {
        self.ctx.resolve_pin_type(pin_id)
    }

    /// 获取类型变量的绑定类型
    pub fn get_bound_type(&self, type_var_id: super::TypeVarId) -> Option<DataType> {
        self.ctx.get_bound_type(type_var_id)
    }
}
