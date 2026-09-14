use yss_ipc_contract::graph_draft::*;

use super::application_event::graph_projection_replacement_to_transport;

pub(crate) fn compile_graph_draft_to_transport(
    receipt: &crate::graph_compile::CompileGraphDraftReceipt,
) -> CompileGraphDraftDto {
    use crate::graph_compile::CompileGraphDraftReceipt;
    match receipt {
        CompileGraphDraftReceipt::Ready {
            artifact_id,
            cache_hit,
            projection,
        } => CompileGraphDraftDto::Ready {
            artifact_id: artifact_id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                .into(),
            cache_hit: *cache_hit,
            projection: crate::ipc::schema::editor_projection::map_editor_projection(projection),
        },
        CompileGraphDraftReceipt::Blocked { projection } => CompileGraphDraftDto::Blocked {
            projection: crate::ipc::schema::editor_projection::map_editor_projection(projection),
        },
    }
}

pub(crate) fn graph_editor_session_to_transport(
    document: &yss_graph_document::GraphDocument,
    projection: &yss_graph_editor::projection::EditorProjectionModel,
) -> GraphEditorSessionDto {
    GraphEditorSessionDto {
        document: document.clone(),
        projection: crate::ipc::schema::editor_projection::map_editor_projection(projection),
    }
}

pub(crate) fn graph_draft_transform_to_transport(
    update: &crate::resource_mutation::GraphDraftTransform,
) -> GraphDraftTransformDto {
    GraphDraftTransformDto {
        changed: update.changed,
        document: update.document.clone(),
        projection: crate::ipc::schema::editor_projection::map_editor_projection(
            &update.projection_replacement.projection,
        ),
    }
}

pub(crate) fn graph_draft_save_to_transport(
    saved: &crate::resource_mutation::GraphDraftSave,
) -> GraphDraftSaveDto {
    GraphDraftSaveDto {
        project_instance_id: saved.project_instance_id.to_string(),
        resource_revision: saved.resource_revision,
        document: saved.document.clone(),
        projection_replacement: graph_projection_replacement_to_transport(
            &saved.projection_replacement,
        ),
    }
}
