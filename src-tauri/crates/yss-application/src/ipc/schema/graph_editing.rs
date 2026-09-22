use yss_ipc_contract::graph_editing::*;

pub(crate) fn graph_edit_receipt_to_transport(
    path: &yss_graph_document::GraphResourcePath,
    receipt: &yss_project::GraphEditCommandReceipt,
) -> GraphEditCommandReceiptDto {
    use yss_project::GraphEditCommandKind;
    GraphEditCommandReceiptDto {
        project_instance_id: receipt.commit.project_instance_id.to_string(),
        graph_path: path.to_string(),
        operation_id: receipt.commit.operation_id.to_string(),
        request_version: GraphEditVersionDto {
            session_id: receipt.request_version.session_id.to_string(),
            revision: receipt.request_version.revision.get().to_string(),
        },
        committed_version: GraphEditVersionDto {
            session_id: receipt.commit.editing.version.session_id.to_string(),
            revision: receipt.commit.to_revision.get().to_string(),
        },
        command: match receipt.kind {
            GraphEditCommandKind::Edit => "edit",
            GraphEditCommandKind::Undo => "undo",
            GraphEditCommandKind::Redo => "redo",
            GraphEditCommandKind::Save => "save",
        },
        changed: receipt.commit.invalidations.graph,
    }
}

pub(crate) fn graph_editor_session_to_transport(
    document: &yss_graph_document::GraphDocument,
    projection: &yss_graph_editor::projection::EditorProjectionModel,
    editing: &yss_project::GraphEditingState,
    result_state: &crate::graph::results::GraphResultState,
) -> Result<GraphEditorSessionDto, crate::ipc::error::CommandError> {
    Ok(GraphEditorSessionDto {
        editing: graph_editing_state_to_transport(editing),
        document: document.clone(),
        projection: crate::ipc::schema::editor_projection::map_editor_projection(projection),
        result_state: super::result::graph_result_state_to_dto(result_state.clone())?,
    })
}

pub(crate) fn graph_editing_state_to_transport(
    state: &yss_project::GraphEditingState,
) -> GraphEditingStateDto {
    GraphEditingStateDto {
        version: GraphEditVersionDto {
            session_id: state.version.session_id.to_string(),
            revision: state.version.revision.get().to_string(),
        },
        dirty: state.dirty,
        can_undo: state.can_undo,
        can_redo: state.can_redo,
    }
}

pub(crate) fn graph_edit_version_from_transport(
    version: GraphEditVersionDto,
) -> Result<yss_project::GraphEditVersion, crate::ipc::error::CommandError> {
    let invalid = || crate::ipc::error::CommandError::expected("invalid_graph_edit_version");
    let session_id = uuid::Uuid::parse_str(&version.session_id).map_err(|_| invalid())?;
    let revision = version.revision.parse::<u64>().map_err(|_| invalid())?;
    if revision.to_string() != version.revision || session_id.to_string() != version.session_id {
        return Err(invalid());
    }
    Ok(yss_project::GraphEditVersion {
        session_id,
        revision: yss_project_identity::ResourceRevision::new(revision),
    })
}

pub(crate) fn encode_graph_session(
    sync: &crate::ipc::graph_editor_sync::GraphEditorSyncState,
    binding: crate::ipc::graph_editor_sync::Binding,
    cursor: Option<&str>,
    session: GraphEditorSessionDto,
) -> Result<GraphEditorSyncResponseDto, crate::ipc::error::CommandError> {
    let update = sync.encode(binding.clone(), cursor, session)?;
    Ok(GraphEditorSyncResponseDto {
        project_instance_id: binding.project,
        graph_path: binding.graph,
        locale: binding.locale,
        changed: false,
        resource_revision: None,
        function_editor_projection: None,
        update,
    })
}

pub(crate) fn encode_graph_edit(
    sync: &crate::ipc::graph_editor_sync::GraphEditorSyncState,
    binding: crate::ipc::graph_editor_sync::Binding,
    cursor: Option<&str>,
    response: &crate::graph::editing::GraphEditResponse,
) -> Result<GraphEditorSyncResponseDto, crate::ipc::error::CommandError> {
    let session = graph_editor_session_to_transport(
        &response.update.document,
        &response.update.projection_replacement.projection,
        &response.editing,
        &response.result_state,
    )?;
    let mut encoded = encode_graph_session(sync, binding, cursor, session)?;
    encoded.changed = response.update.changed;
    encoded.function_editor_projection = response
        .update
        .projection_replacement
        .function_editor_projection
        .clone();
    Ok(encoded)
}
