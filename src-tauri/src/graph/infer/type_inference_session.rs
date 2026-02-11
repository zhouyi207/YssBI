use super::TypeInferenceContext;
use crate::graph::{GraphInstance, PinId, PinState};
use std::collections::VecDeque;

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
        // 遍历节点
        let pins = self.graph.data_state.read().unwrap().pins;
        
        for (pin_id, pin) in pins.iter() {
            /// 在这里必定报错,没有考虑 exec 
            self.ctx.register_pin_type(pin_id.clone(), pin.definition.data_type.clone().unwrap());
        }
        
        
        
        
        
        
        for node in nodes.values() {
            // 注册节点里的类型变量
            for var in node.type_vars() {
                self.ctx.register_type_var(var.clone());
            }

            // 注册节点的 pin 类型
            for pin in node.pins() {
                self.ctx.register_pin_type(pin.id, pin.data_type.clone());
            }
        }
    }

    /// 推断整张图（全量）
    pub fn infer_all(&mut self) -> Result<(), String> {
        let connections = self.graph.connections.all_connections();
        for (from, to) in connections {
            self.ctx.infer_connection(from, to)?;
        }
        Ok(())
    }

    /// 推断增量（只推断脏的连接/Pin）
    /// 使用 worklist 遍历依赖传播类型
    pub fn infer_incremental(&mut self, dirty_pins: Vec<PinId>) -> Result<(), String> {
        let mut queue: VecDeque<PinId> = dirty_pins.into();

        while let Some(pin) = queue.pop_front() {
            let neighbors = self.graph.connections.connected_to(pin);
            for &neighbor in neighbors.iter() {
                // 如果 unify 失败就返回
                if self.ctx.infer_connection(pin, neighbor).is_ok() {
                    queue.push_back(neighbor);
                }
            }
        }
        Ok(())
    }

    /// 提交结果：把临时绑定写回 TypeVarDefinition.bound
    pub fn commit(self) -> Result<(), String> {
        self.ctx.commit()
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
