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
    pub inference_warnings: Vec<GraphValidationWarning>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphValidationWarning {
    pub code: &'static str,
    pub from_pin_id: PinId,
    pub to_pin_id: PinId,
    pub message: String,
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

/// 函数签名 pin：`data_type == None` 表示 exec；data pin 直接携带结构化 `DataType`。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSignaturePin {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_type: Option<DataType>,
}

impl FunctionSignaturePin {
    pub fn is_exec(&self) -> bool {
        self.data_type.is_none()
    }

    pub fn data(sig_id: impl Into<String>, name: impl Into<String>, data_type: DataType) -> Self {
        Self {
            id: sig_id.into(),
            name: name.into(),
            data_type: Some(data_type),
        }
    }

    pub fn exec(sig_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: sig_id.into(),
            name: name.into(),
            data_type: None,
        }
    }
}

/// 新建函数图默认 exec 入参签名 id。
pub const DEFAULT_FUNCTION_EXEC_IN_ID: &str = "exec-in";
/// 新建函数图默认 exec 出参签名 id。
pub const DEFAULT_FUNCTION_EXEC_OUT_ID: &str = "exec-out";

pub fn default_function_exec_input() -> FunctionSignaturePin {
    FunctionSignaturePin::exec(DEFAULT_FUNCTION_EXEC_IN_ID, "In")
}

pub fn default_function_exec_output() -> FunctionSignaturePin {
    FunctionSignaturePin::exec(DEFAULT_FUNCTION_EXEC_OUT_ID, "Out")
}

pub fn default_function_exec_inputs() -> Vec<FunctionSignaturePin> {
    vec![default_function_exec_input()]
}

pub fn default_function_exec_outputs() -> Vec<FunctionSignaturePin> {
    vec![default_function_exec_output()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_pin_serializes_structured_data_type() {
        let pin = FunctionSignaturePin::data(
            "a",
            "Value",
            DataType::DataSeries(Box::new(DataType::Float64)),
        );
        let json = serde_json::to_value(&pin).unwrap();
        assert_eq!(json["dataType"]["kind"], "DataSeries");
        assert_eq!(json["dataType"]["inner"]["kind"], "Float64");
        assert!(json.get("type").is_none());
        assert!(json.get("containerType").is_none());
    }

    #[test]
    fn exec_signature_omits_data_type() {
        let pin = default_function_exec_input();
        let json = serde_json::to_value(&pin).unwrap();
        assert!(json.get("dataType").is_none());
    }
}
