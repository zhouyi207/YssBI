use crate::error::{CommandError, GraphMutationErrorDetailsDto};
use crate::event::{Event, EventProject};
use crate::schema::application_event::ResourceMutationResultDto;
use serde::Serialize;
use yss_graph_document::GraphResourcePath;

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
    error: crate::graph::document::MutationConflict,
    revision_conflict_code: &'static str,
) -> CommandError {
    match error {
        crate::graph::document::MutationConflict::RecoveryRequired(_) => CommandError::expected(
            "project_recovery_required",
        )
        .with_details(RecoveryRequiredDetails {
            recovery_required: true,
        }),
        public_error @ (crate::graph::document::MutationConflict::CatalogResourceStale(_)
        | crate::graph::document::MutationConflict::CatalogDescriptorInvalid(_)
        | crate::graph::document::MutationConflict::ClipboardSubgraphInvalid(_)
        | crate::graph::document::MutationConflict::ReferencedResourceUnavailable(
            _,
        )) => CommandError::expected(public_error.code()),
        crate::graph::document::MutationConflict::Document(
            crate::graph::document::DocumentError::ConnectionNotFound(_),
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
        crate::graph::document::MutationConflict::Editor(error) => {
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
        crate::graph::document::MutationConflict::StaleRevision { .. } => {
            let command_error = CommandError::expected(revision_conflict_code);
            if revision_conflict_code == "graph_revision_conflict" {
                command_error.with_details(GraphMutationErrorDetailsDto::VALUE)
            } else {
                command_error
            }
        }
        crate::graph::document::MutationConflict::StaleProjectLifecycle(_) => {
            CommandError::expected("stale_project_lifecycle")
        }
        _ => CommandError::internal(error),
    }
}

pub(super) fn resource_mutation_to_command_error(
    error: crate::application::resource_mutation::ResourceMutationApplicationError,
    revision_conflict_code: &'static str,
) -> CommandError {
    use crate::application::resource_mutation::ResourceMutationApplicationError;
    match error {
        ResourceMutationApplicationError::SessionCapture(error) => match error {
            crate::application::execution::SessionCaptureError::Inactive => {
                CommandError::expected("stale_project_lifecycle")
            }
            crate::application::execution::SessionCaptureError::Replacing => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            crate::application::execution::SessionCaptureError::Recovering => {
                CommandError::expected("project_recovery_required")
                    .with_details(RecoveryRequiredDetails {
                        recovery_required: true,
                    })
            }
        },
        ResourceMutationApplicationError::Project(error) => CommandError::from(error),
        ResourceMutationApplicationError::Mutation(error) => {
            mutation_conflict_to_command_error(error, revision_conflict_code)
        }
        ResourceMutationApplicationError::History(error) => match error {
            crate::project::ProjectHistoryMutationError::StaleProjectLifecycle(_) => {
                CommandError::expected("stale_project_lifecycle")
            }
            crate::project::ProjectHistoryMutationError::RecoveryRequired(_) => {
                CommandError::expected("project_recovery_required").with_details(
                    RecoveryRequiredDetails {
                        recovery_required: true,
                    },
                )
            }
            crate::project::ProjectHistoryMutationError::StaleRevision { .. } => {
                CommandError::expected(revision_conflict_code)
            }
            crate::project::ProjectHistoryMutationError::ResourceMismatch { .. } => {
                CommandError::expected("history_resource_mismatch")
            }
            crate::project::ProjectHistoryMutationError::Projection(_)
            | crate::project::ProjectHistoryMutationError::History(_) => {
                CommandError::diagnosed("history_mutation_failed", error)
            }
        },
        ResourceMutationApplicationError::GraphOperation(error) => {
            match error {
                crate::project::project_state::graph_operation::ProjectGraphOperationError::ProjectIdentityMismatch { .. } => {
                    CommandError::expected("stale_project_lifecycle")
                }
                crate::project::project_state::graph_operation::ProjectGraphOperationError::GraphUnavailable { .. } => {
                    CommandError::internal("graph resource is unavailable")
                }
                crate::project::project_state::graph_operation::ProjectGraphOperationError::RevisionConflict { .. } => {
                    let command = CommandError::expected(revision_conflict_code);
                    if revision_conflict_code == "graph_revision_conflict" {
                        command.with_details(GraphMutationErrorDetailsDto::VALUE)
                    } else {
                        command
                    }
                }
                crate::project::project_state::graph_operation::ProjectGraphOperationError::ResourceLifecycleChanged { .. } => {
                    CommandError::expected("stale_resource_lifecycle")
                }
                crate::project::project_state::graph_operation::ProjectGraphOperationError::OperationOwnershipChanged { .. } => {
                    CommandError::expected("duplicate_operation")
                }
                crate::project::project_state::graph_operation::ProjectGraphOperationError::AdmissionClosed => {
                    CommandError::expected("project_lifecycle_admission_closed")
                }
                crate::project::project_state::graph_operation::ProjectGraphOperationError::RecoveryRequired => {
                    CommandError::expected("project_recovery_required").with_details(
                        RecoveryRequiredDetails {
                            recovery_required: true,
                        },
                    )
                }
                crate::project::project_state::graph_operation::ProjectGraphOperationError::Internal(error) => {
                    CommandError::internal(error)
                }
            }
        }
        ResourceMutationApplicationError::GraphCommit(error) => match error {
            crate::project::project_state::graph_operation::ProjectGraphCommitError::StaleAuthority { .. } => {
                let command = CommandError::expected(revision_conflict_code);
                if revision_conflict_code == "graph_revision_conflict" {
                    command.with_details(GraphMutationErrorDetailsDto::VALUE)
                } else {
                    command
                }
            }
            crate::project::project_state::graph_operation::ProjectGraphCommitError::RevisionExhausted { .. } => {
                CommandError::expected("resource_revision_overflow")
            }
            crate::project::project_state::graph_operation::ProjectGraphCommitError::LifecycleChanged { .. } => {
                CommandError::expected("stale_resource_lifecycle")
            }
            crate::project::project_state::graph_operation::ProjectGraphCommitError::OperationOwnershipChanged { .. } => {
                CommandError::expected("duplicate_operation")
            }
        },
        ResourceMutationApplicationError::GraphApplication(error) => CommandError::internal(error),
        ResourceMutationApplicationError::Catalog(error) => {
            CommandError::diagnosed("catalog_project_read_failed", error)
        }
        ResourceMutationApplicationError::Database(error) => {
            CommandError::diagnosed("database_catalog_failed", error)
        }
        ResourceMutationApplicationError::Contract(error) => {
            CommandError::diagnosed("graph_contract_failed", error)
        }
        ResourceMutationApplicationError::Projection(error) => {
            CommandError::diagnosed("editor_projection_failed", error)
        }
        ResourceMutationApplicationError::SessionChanged(error) => {
            CommandError::diagnosed("resource_session_changed", error)
        }
        ResourceMutationApplicationError::SessionRefresh(error) => {
            CommandError::diagnosed("resource_session_refresh_failed", error)
        }
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
