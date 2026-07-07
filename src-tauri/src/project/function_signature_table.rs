//! 项目级函数签名表：Call 投影 / Detail 的读路径单一事实来源（与磁盘 function 图文件写入对齐）。
//!
//! - **读**：优先已加载 `GraphInstance`，其次内存表（由 `read_project_index` 填充），最后图文件头。
//! - **写**：`update_function_signature` / 函数图持久化后 `upsert`；函数删除时 `remove`。

use crate::graph::{FunctionSignaturePin, GraphId, GraphInstance, GraphKind};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FunctionSignatureEntry {
    pub inputs: Vec<FunctionSignaturePin>,
    pub outputs: Vec<FunctionSignaturePin>,
}

#[derive(Clone, Debug, Default)]
pub struct FunctionSignatureTable {
    entries: HashMap<GraphId, FunctionSignatureEntry>,
}

impl FunctionSignatureTable {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn upsert(
        &mut self,
        id: GraphId,
        inputs: Vec<FunctionSignaturePin>,
        outputs: Vec<FunctionSignaturePin>,
    ) {
        self.entries.insert(id, FunctionSignatureEntry { inputs, outputs });
    }

    pub fn upsert_from_graph(&mut self, graph: &GraphInstance) {
        if graph.kind != GraphKind::Function {
            return;
        }
        self.upsert(
            graph.id,
            graph.function_inputs.clone(),
            graph.function_outputs.clone(),
        );
    }

    pub fn get(&self, id: &GraphId) -> Option<&FunctionSignatureEntry> {
        self.entries.get(id)
    }

    pub fn get_cloned(&self, id: &GraphId) -> Option<FunctionSignatureEntry> {
        self.entries.get(id).cloned()
    }

    pub fn remove(&mut self, id: &GraphId) {
        self.entries.remove(id);
    }
}
