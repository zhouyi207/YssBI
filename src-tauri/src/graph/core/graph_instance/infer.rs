use super::*;

/// 类型推断
impl GraphInstance {
    /// 运行类型推断
    ///
    /// 这个方法会：
    /// 1. 注册所有节点的类型变量
    /// 2. 注册所有 Pin 的类型
    /// 3. 根据连接关系推断类型
    /// 4. 将推断结果写回 GraphDataState
    pub fn infer_types(&self) -> Result<Vec<(PinId, DataType)>, String> {
        crate::graph::infer::infer_graph(self)
    }
}
