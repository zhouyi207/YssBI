use super::TypeInferenceContext;
use crate::graph::pin::PinDataTypeInference;
use crate::graph::{DataType, GraphInstance, PinId};

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
        
        // 1. 先注册所有节点的类型变量
        for node_instance in data_state.nodes.values() {
            for (&type_var_id, type_var_def) in &node_instance.type_var_map {
                let type_var_inference = super::TypeVarInference {
                    id: type_var_id,
                    constraints: type_var_def.constraints.clone(),
                    bound: type_var_def.bound.clone(),
                };
                self.ctx.register_type_var(type_var_inference);
            }
        }
        
        // 2. 然后注册所有 Pin（只注册有类型描述的 Data Pin）
        for pin_instance in data_state.pins.values() {
            self.ctx.register_pin_type(pin_instance.clone());
        }
    }

    /// 推断整张图（全量）
    pub fn infer_all(&mut self) -> Result<(), String> {
        let connections = self
            .graph
            .data_state
            .read()
            .unwrap()
            .connections
            .all_connections();
        for connection in connections {
            self.ctx
                .infer_connection(connection.from_pin, connection.to_pin)?;
        }
        Ok(())
    }

    // /// 推断增量（只推断脏的连接/Pin）
    // /// 使用 worklist 遍历依赖传播类型
    // pub fn infer_incremental(&mut self, dirty_pins: Vec<PinId>) -> Result<(), String> {
    //     let mut queue: VecDeque<PinId> = dirty_pins.into();

    //     while let Some(pin) = queue.pop_front() {
    //         let neighbors = self
    //             .graph
    //             .data_state
    //             .read()
    //             .unwrap()
    //             .connections
    //             .connected_to(pin);
    //         for &neighbor in neighbors.iter() {
    //             // 如果 unify 失败就返回
    //             if self.ctx.infer_connection(pin, neighbor).is_ok() {
    //                 queue.push_back(neighbor);
    //             }
    //         }
    //     }
    //     Ok(())
    // }

    /// 提交结果：把临时绑定写回 TypeVarDefinition.bound
    pub fn commit(&mut self) -> Result<(), String> {
        self.ctx.commit()
    }

    /// 提交推断结果到 graph data_state，返回所有被写入的 (PinId, DataType) 列表
    pub fn commit_to_graph(&mut self) -> Result<Vec<(PinId, DataType)>, String> {
        let mut data_state = self.graph.data_state.write().unwrap();
        let mut resolved = Vec::new();

        for (&pin_id, pin_data) in self.ctx.pin_types.iter() {
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

        // 写回 TypeVar 绑定，清理不再绑定的旧条目
        for (&var_id, var_def) in self.ctx.type_vars.iter() {
            if let Some(bound_type) = &var_def.bound {
                data_state
                    .type_var_bindings
                    .insert(var_id, bound_type.clone());
            } else if self.ctx.bindings.get(&var_id).is_none() {
                data_state.type_var_bindings.remove(&var_id);
            }
        }

        self.ctx.commit()?;

        Ok(resolved)
    }

    /// 查询某个 pin 的最终类型
    pub fn resolve_pin(&self, pin_id: PinId) -> Result<DataType, String> {
        self.ctx.resolve_pin_type(pin_id)
    }

    /// 获取类型变量的绑定类型
    pub fn get_bound_type(&self, type_var_id: super::TypeVarId) -> Option<DataType> {
        self.ctx.get_bound_type(type_var_id)
    }
}
