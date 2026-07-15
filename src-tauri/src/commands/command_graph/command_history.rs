//! History / Undo-Redo 相关命令
//!
//! 提供图状态同步命令，用于 undo/redo 后将前端快照重建到后端。

use crate::project::ProjectState;
use crate::error::AppError;
use crate::schema::GraphRebuildSnapshot;
use tauri::State;

/// 从前端快照同步后端 Graph 状态（undo/redo 后调用）
///
/// 前端在执行 undo/redo 后，将目标快照发送给后端，
/// 后端清空当前 Graph 并从快照重建，确保前后端状态一致。
#[tauri::command]
pub fn sync_graph_state(
    state: State<ProjectState>,
    graph_path: String,
    snapshot: GraphRebuildSnapshot,
) -> Result<(), AppError> {
    let graph_path = crate::project::GraphResourcePath::new(graph_path).map_err(AppError::from)?;
    Ok(state.with_graph_mut(&graph_path, |mut ctx| {
        ctx.graph().rebuild_from_snapshot(snapshot)?;
        ctx.sync_runtime_symbols();
        Ok(())
    })?)
}
