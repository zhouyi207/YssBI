use crate::error::{CommandError, GraphMutationErrorDetailsDto};
use crate::event::{Event, EventProject, ResourceMutationResultDto};
use crate::graph_document::GraphResourcePath;
use serde::Serialize;

pub(super) fn parse_graph_path(value: String) -> Result<GraphResourcePath, CommandError> {
    GraphResourcePath::new(value).map_err(|_| CommandError::expected("invalid_project_format"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorFieldDetails {
    field: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecoveryRequiredDetails {
    pub(super) recovery_required: bool,
}

pub(super) fn parse_opaque_u64(field: &'static str, value: &str) -> Result<u64, CommandError> {
    let canonical = !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && !value.starts_with('0');
    canonical
        .then(|| value.parse::<u64>().ok())
        .flatten()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            CommandError::expected("invalid_opaque_id").with_details(ErrorFieldDetails { field })
        })
}

pub(super) fn mutation_conflict_to_command_error(
    error: crate::node_system::document::MutationConflict,
    revision_conflict_code: &'static str,
) -> CommandError {
    match error {
        crate::node_system::document::MutationConflict::RecoveryRequired(_) => {
            CommandError::expected("project_recovery_required").with_details(
                RecoveryRequiredDetails {
                    recovery_required: true,
                },
            )
        }
        public_error
        @ (crate::node_system::document::MutationConflict::CatalogResourceStale(_)
        | crate::node_system::document::MutationConflict::CatalogDescriptorInvalid(_)
        | crate::node_system::document::MutationConflict::ClipboardSubgraphInvalid(_)
        | crate::node_system::document::MutationConflict::ReferencedResourceUnavailable(_)) => {
            CommandError::expected(public_error.code())
        }
        crate::node_system::document::MutationConflict::Document(
            crate::node_system::document::DocumentError::ConnectionNotFound(_),
        ) => {
            tracing::warn!(
                target: "yssbi::node_system::graph_mutation",
                diagnostic_domain = "graph",
                diagnostic_event = "mutationRejected",
                error = %error,
                "Graph mutation rejected"
            );
            CommandError::expected("graph_connection_not_found")
                .with_details(GraphMutationErrorDetailsDto::VALUE)
        }
        crate::node_system::document::MutationConflict::Editor(error) => {
            tracing::warn!(
                target: "yssbi::node_system::graph_mutation",
                diagnostic_domain = "graph",
                diagnostic_event = "mutationRejected",
                error_code = error.code.as_str(),
                detail = error.detail,
                "Graph mutation rejected"
            );
            CommandError::expected(error.code.as_str())
                .with_details(GraphMutationErrorDetailsDto::VALUE)
        }
        crate::node_system::document::MutationConflict::StaleRevision { .. } => {
            let command_error = CommandError::expected(revision_conflict_code);
            if revision_conflict_code == "graph_revision_conflict" {
                command_error.with_details(GraphMutationErrorDetailsDto::VALUE)
            } else {
                command_error
            }
        }
        crate::node_system::document::MutationConflict::StaleProjectLifecycle(_) => {
            CommandError::expected("stale_project_lifecycle")
        }
        _ => CommandError::internal(error),
    }
}

pub(super) trait EmitOutcome {
    fn discard(self);
}

impl EmitOutcome for () {
    fn discard(self) {}
}

impl<E> EmitOutcome for Result<(), E> {
    fn discard(self) {}
}

pub(super) fn emit_resource_result<R: EmitOutcome>(
    emit: &mut impl FnMut(Event) -> R,
    result: &ResourceMutationResultDto,
) {
    emit(Event::Project(EventProject::ResourceMutationCommitted {
        result: result.clone(),
    }))
    .discard();
}
