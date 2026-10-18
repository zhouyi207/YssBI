use yss_function_editor_projection::FunctionEditorProjection;
use yss_graph_document::GraphResourcePath;
use yss_graph_editor::projection::EditorProjectionModel;
use yss_project_history::{ResourceDeltaEvent, ResourceDocumentPatch, ResourceLifecycleKind};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_project_registry_contract::ProjectRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLifecycleApplicationEvent {
    pub operation_id: OperationId,
    pub kind: ProjectLifecycleKind,
    pub old_project_instance_id: Option<ProjectInstanceId>,
    pub new_project_instance_id: Option<ProjectInstanceId>,
    pub phase: ProjectLifecyclePhase,
    pub outcome: ProjectLifecycleOutcome,
    pub record: Option<ProjectRecord>,
    pub path: Option<Box<str>>,
    pub recovery: Option<LifecycleRecovery>,
    pub invalidation: LifecycleInvalidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectLifecycleKind {
    SaveAs,
    Create,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectLifecyclePhase {
    DestinationCommitted,
    RegistryCommitted,
    AuthorityCommitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectLifecycleOutcome {
    Committed,
    RegistryFailed,
    ActivationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleRecovery {
    pub required: bool,
    pub action: LifecycleRecoveryAction,
    pub path: Option<Box<str>>,
    pub identity: Option<Box<str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleRecoveryAction {
    RegisterDestination,
    ActivateDestination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleInvalidation {
    pub project: bool,
    pub registry: bool,
}

/// Committed resource facts shared by command receipts and publication events.
#[derive(Debug, Clone, PartialEq)]
pub struct CommittedResourceMutation {
    pub operation_id: OperationId,
    pub project_instance_id: ProjectInstanceId,
    pub publication_revision: u64,
    pub moves: Vec<ResourceMove>,
    pub deltas: Vec<ResourceDeltaEvent>,
    pub projection_status: ResourceProjectionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceMove {
    pub from: Box<str>,
    pub to: Box<str>,
    pub kind: ResourceLifecycleKind,
    pub name: Box<str>,
}

/// How one committed mutation reaches the editor open at a resource path.
#[derive(Debug, PartialEq)]
pub enum ResourceRoute<'a> {
    Moved {
        from: &'a str,
        to: &'a str,
        from_revision: ResourceRevision,
        to_revision: ResourceRevision,
    },
    Removed {
        path: &'a str,
        revision: ResourceRevision,
    },
    Changed {
        path: &'a str,
        delta: &'a ResourceDeltaEvent,
    },
}

impl CommittedResourceMutation {
    /// Moves come first so that later routes in the same mutation address the moved editor.
    pub fn routes(&self) -> Vec<ResourceRoute<'_>> {
        let moves = self.moves.iter().filter_map(|moved| {
            // Graphs that reference a moved graph carry the same move patch;
            // only the destination's own delta versions the move.
            let delta = self.deltas.iter().find(|delta| {
                delta.resource.path() == Some(moved.to.as_ref())
                    && matches!(&delta.payload, ResourceDocumentPatch::ResourceMove(patch)
                        if patch.from == moved.from && patch.to == moved.to)
            })?;
            Some(ResourceRoute::Moved {
                from: &moved.from,
                to: &moved.to,
                from_revision: delta.from_revision,
                to_revision: delta.to_revision,
            })
        });
        let changes = self.deltas.iter().filter_map(|delta| {
            let path = delta.resource.path()?;
            match &delta.payload {
                ResourceDocumentPatch::ResourceMove(_) => None,
                ResourceDocumentPatch::ResourceLifecycle(patch) if patch.after.is_none() => {
                    Some(ResourceRoute::Removed {
                        path,
                        revision: delta.from_revision,
                    })
                }
                _ => Some(ResourceRoute::Changed { path, delta }),
            }
        });
        moves.chain(changes).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceProjectionStatus {
    Complete {
        expected_graph_paths: Vec<GraphResourcePath>,
    },
    Incomplete {
        invalidated_graph_paths: Vec<GraphResourcePath>,
    },
}

impl ResourceProjectionStatus {
    pub fn affected_graph_paths(&self) -> &[GraphResourcePath] {
        match self {
            Self::Complete {
                expected_graph_paths,
            } => expected_graph_paths,
            Self::Incomplete {
                invalidated_graph_paths,
            } => invalidated_graph_paths,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphProjectionReplacement {
    pub graph_path: Box<str>,
    pub projection: EditorProjectionModel,
    pub function_editor_projection: Option<FunctionEditorProjection>,
}

pub(crate) fn committed_resource_mutation_from_project(
    facts: yss_project::project_writers::ProjectResourceMutationFacts,
) -> CommittedResourceMutation {
    let yss_project::project_writers::ProjectResourceMutationParts {
        operation_id,
        project_instance_id,
        publication_revision,
        moves,
        deltas,
        projection_status,
    } = facts.into_parts();
    CommittedResourceMutation {
        operation_id,
        project_instance_id,
        publication_revision,
        moves: moves
            .into_vec()
            .into_iter()
            .map(|value| ResourceMove {
                from: value.from,
                to: value.to,
                kind: value.kind,
                name: value.name,
            })
            .collect(),
        deltas: deltas.into_vec(),
        projection_status: match projection_status {
            yss_project::project_writers::ProjectProjectionStatus::Complete {
                expected_graph_paths,
            } => ResourceProjectionStatus::Complete {
                expected_graph_paths: expected_graph_paths.into_vec(),
            },
            yss_project::project_writers::ProjectProjectionStatus::Incomplete {
                invalidated_graph_paths,
            } => ResourceProjectionStatus::Incomplete {
                invalidated_graph_paths: invalidated_graph_paths.into_vec(),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_project_history::{
        DocResourceKey, ResourceKey, ResourceLifecyclePatch, ResourceLifecycleState,
        ResourcePathMovePatch,
    };

    fn delta(
        resource: ResourceKey,
        from: u64,
        to: u64,
        payload: ResourceDocumentPatch,
    ) -> ResourceDeltaEvent {
        ResourceDeltaEvent {
            resource,
            from_revision: ResourceRevision::new(from),
            to_revision: ResourceRevision::new(to),
            caused_by: None,
            payload,
        }
    }

    fn graph(path: &str) -> ResourceKey {
        ResourceKey::Graph(GraphResourcePath::new(path).unwrap())
    }

    fn moved(from: &str, to: &str) -> ResourceDocumentPatch {
        ResourceDocumentPatch::ResourceMove(ResourcePathMovePatch {
            from: from.into(),
            to: to.into(),
        })
    }

    fn mutation(
        moves: Vec<ResourceMove>,
        deltas: Vec<ResourceDeltaEvent>,
    ) -> CommittedResourceMutation {
        CommittedResourceMutation {
            operation_id: OperationId::new(),
            project_instance_id: ProjectInstanceId::new(),
            publication_revision: 1,
            moves,
            deltas,
            projection_status: ResourceProjectionStatus::Complete {
                expected_graph_paths: vec![],
            },
        }
    }

    #[test]
    fn a_move_is_versioned_by_its_destination_not_by_graphs_that_reference_it() {
        let from = "functions/Total.yssbi-function";
        let to = "functions/Sum.yssbi-function";
        let mutation = mutation(
            vec![ResourceMove {
                from: from.into(),
                to: to.into(),
                kind: ResourceLifecycleKind::FunctionGraph,
                name: "Sum".into(),
            }],
            vec![
                delta(graph("events/Report.yssbi-event"), 1, 2, moved(from, to)),
                delta(graph(to), 5, 6, moved(from, to)),
            ],
        );
        assert_eq!(
            mutation.routes(),
            [ResourceRoute::Moved {
                from,
                to,
                from_revision: ResourceRevision::new(5),
                to_revision: ResourceRevision::new(6),
            }]
        );
    }

    #[test]
    fn deletions_carry_their_last_revision_and_other_patches_are_changes() {
        let state = |revision| ResourceLifecycleState {
            revision: ResourceRevision::new(revision),
            path: "docs/Notes.md".into(),
            kind: ResourceLifecycleKind::Doc,
            name: "Notes".into(),
        };
        let lifecycle = |before, after| {
            ResourceDocumentPatch::ResourceLifecycle(ResourceLifecyclePatch { before, after })
        };
        let doc = || ResourceKey::Doc(DocResourceKey("docs/Notes.md".into()));
        let edited = delta(doc(), 3, 4, lifecycle(Some(state(3)), Some(state(4))));
        let removed = delta(doc(), 4, 5, lifecycle(Some(state(4)), None));
        let mutation = mutation(vec![], vec![edited.clone(), removed]);
        assert_eq!(
            mutation.routes(),
            [
                ResourceRoute::Changed {
                    path: "docs/Notes.md",
                    delta: &edited,
                },
                ResourceRoute::Removed {
                    path: "docs/Notes.md",
                    revision: ResourceRevision::new(4),
                },
            ]
        );
    }
}
