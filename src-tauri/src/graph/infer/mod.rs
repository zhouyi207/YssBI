pub mod type_constraint;
pub mod type_inference_context;
pub mod type_inference_session;
pub mod type_var_definition;
pub mod type_var_id;
pub mod type_var_inference;
pub mod type_var_key;

pub use type_constraint::*;
pub use type_inference_context::*;
pub use type_inference_session::*;
pub use type_var_definition::*;
pub use type_var_id::*;
pub use type_var_inference::*;
pub use type_var_key::*;

use crate::graph::{DataType, GraphInstance, PinId};

/// 执行全量类型推断，返回所有被解析的 (PinId, DataType)
pub fn infer_graph(graph_instance: &GraphInstance) -> Result<Vec<(PinId, DataType)>, String> {
    let mut session = TypeInferenceSession::new(graph_instance);
    session.register_all();
    session.infer_all()?;
    let resolved = session.commit_to_graph()?;
    Ok(resolved)
}
