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

use crate::graph::GraphInstance;

pub fn infer_graph(graph_instance: &GraphInstance) -> Result<(), String> {
    let mut session = TypeInferenceSession::new(graph_instance);
    // 2. 注册图中所有 pin/类型变量
    session.register_all();
    // 3. 全量推断
    session.infer_all()?;
    // 4. 提交结果并写回 graph cache
    session.commit_to_graph()?;

    Ok(())
}
