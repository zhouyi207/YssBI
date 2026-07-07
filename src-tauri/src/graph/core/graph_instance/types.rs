use crate::graph::pin::{PinId, PinInstance};
use crate::graph::value::DataType;
use crate::graph::NodeId;
use serde::{Deserialize, Serialize};

/// Post-mutation graph compile scope — single entry for schema propagation,
/// dynamic pin resolution, and type inference.
#[derive(Clone, Debug)]
pub enum GraphRecompileScope {
    /// `insert_graph` / load path: propagate + infer, no dynamic pin materialization.
    RuntimePrepare,
    /// Full graph: propagate + all dynamic pins + infer.
    Full,
    /// Variable or localized change: partial propagate + infer + downstream dynamic pins.
    FromSeeds(Vec<NodeId>),
    /// Connection topology: partial propagate + infer + dynamic pins (with mode).
    TopologyEffects {
        seeds: Vec<NodeId>,
        mode: PinResolveMode,
    },
    /// Type inference only (isolated node create/delete without topology change).
    InferOnly,
    /// Open tab: propagate + materialize dynamic pins + infer.
    Materialize,
    /// No post-mutation compile step.
    None,
}

/// Side effects collected by [`GraphInstance::recompile`].
#[derive(Clone, Debug, Default)]
pub struct GraphRecompileResult {
    pub change_sets: Vec<PinChangeSet>,
    pub inferred: Vec<(PinId, DataType)>,
}

/// 动态 pin 解析模式
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinResolveMode {
    /// 连线 / 断线 / 改参：允许移除过时的 schema 派生 pin
    Interactive,
    /// 打开 Tab 物化：resolver 无结果时保留已持久化的 pin
    Materialize,
}

/// 动态 pin 重建的变更集
#[derive(Debug, Clone, Default)]
pub struct PinChangeSet {
    pub node_id: NodeId,
    pub removed_pin_ids: Vec<PinId>,
    pub added_pins: Vec<PinInstance>,
    /// 同族 repeatable pin 重排索引后需同步到前端的 pin（如 C→B）
    pub updated_pins: Vec<PinInstance>,
    pub removed_connections: Vec<(PinId, PinId)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSignaturePin {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_type: Option<String>,
}

impl FunctionSignaturePin {
    pub fn is_exec(&self) -> bool {
        self.pin_type.eq_ignore_ascii_case("exec")
    }
}

/// 新建函数图默认 exec 入参签名 id。
pub const DEFAULT_FUNCTION_EXEC_IN_ID: &str = "exec-in";
/// 新建函数图默认 exec 出参签名 id。
pub const DEFAULT_FUNCTION_EXEC_OUT_ID: &str = "exec-out";

pub fn default_function_exec_input() -> FunctionSignaturePin {
    FunctionSignaturePin {
        id: DEFAULT_FUNCTION_EXEC_IN_ID.to_string(),
        name: "In".to_string(),
        pin_type: "exec".to_string(),
        container_type: None,
    }
}

pub fn default_function_exec_output() -> FunctionSignaturePin {
    FunctionSignaturePin {
        id: DEFAULT_FUNCTION_EXEC_OUT_ID.to_string(),
        name: "Out".to_string(),
        pin_type: "exec".to_string(),
        container_type: None,
    }
}

pub fn default_function_exec_inputs() -> Vec<FunctionSignaturePin> {
    vec![default_function_exec_input()]
}

pub fn default_function_exec_outputs() -> Vec<FunctionSignaturePin> {
    vec![default_function_exec_output()]
}
