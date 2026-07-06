use tauri::State;

/// 读取 source descriptor。
#[tauri::command]
pub fn get_result_source_descriptor(
    state: State<crate::execution::ResultSourceStore>,
    source_id: String,
) -> Result<Option<crate::execution::SourceDescriptor>, String> {
    Ok(state.get_descriptor(&source_id))
}

/// 通过 graphId + pinId 读取最新 runtime pin source descriptor。
#[tauri::command]
pub fn get_pin_result_descriptor(
    state: State<crate::execution::ResultSourceStore>,
    graph_id: String,
    pin_id: String,
) -> Result<Option<crate::execution::SourceDescriptor>, String> {
    Ok(state.get_pin_descriptor(&graph_id, &pin_id))
}

/// 读取 JSON source value。
#[tauri::command]
pub fn get_result_source_value(
    state: State<crate::execution::ResultSourceStore>,
    source_id: String,
) -> Result<Option<crate::execution::SourceValue>, String> {
    state.get_value(&source_id)
}

/// 分页拉取 source 中的 DataFrame / DataSeries 数据。
#[tauri::command]
pub fn get_result_source_page(
    state: State<crate::execution::ResultSourceStore>,
    source_id: String,
    offset: usize,
    limit: usize,
) -> Result<crate::execution::SourcePage, String> {
    state.get_page(&source_id, offset, limit)
}

/// Release a window-owned result source when its view window unmounts.
#[tauri::command]
pub fn release_result_source(
    state: State<'_, crate::execution::ResultSourceStore>,
    source_id: String,
) -> Result<(), String> {
    state.release_window_source(&source_id)?;
    Ok(())
}
