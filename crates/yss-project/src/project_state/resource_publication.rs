use super::*;
use yss_project_model::ProjectDataPatch;

pub(crate) fn validate_context_revisions(
    context: &ProjectTransactionContext,
    data: &ProjectData,
    graph_resource_revisions: &std::collections::HashMap<GraphResourcePath, ResourceRevision>,
) -> Result<(), ProjectOperationError> {
    for resource in &context.affected_resources {
        let expected = context.expected_revisions.get(resource).ok_or_else(|| {
            ProjectOperationError::ResourceRevisionConflict {
                message: format!("missing expected revision for {resource:?}"),
            }
        })?;
        let actual = match resource {
            ResourceKey::Graph(path) => Some(
                graph_resource_revisions
                    .get(path)
                    .copied()
                    .unwrap_or(ResourceRevision::INITIAL),
            ),
            ResourceKey::Function(path) => GraphResourcePath::new(path.0.as_ref())
                .ok()
                .and_then(|path| data.graphs.get(&path))
                .and_then(|resource| resource.function.as_ref())
                .map(|function| function.revision),
            ResourceKey::Database(_) => None,
            ResourceKey::Mind(path) => yss_project_model::mind::MindPath::parse(&path.0)
                .ok()
                .and_then(|path| data.minds.get(&path))
                .map(|doc| doc.version.revision),
            ResourceKey::Doc(path) => yss_project_model::doc::DocPath::parse(&path.0)
                .ok()
                .and_then(|path| data.docs.get(&path))
                .map(|doc| doc.version.revision),
            ResourceKey::Chart(path) => yss_project_model::chart::ChartPath::parse(&path.0)
                .ok()
                .and_then(|path| data.charts.get(&path))
                .map(|chart| chart.version.revision),
        };
        if actual != Some(*expected) {
            return Err(ProjectOperationError::ResourceRevisionConflict {
                message: format!(
                    "revision for {resource:?} changed from {} to {}",
                    expected.get(),
                    actual
                        .map(|revision| revision.get().to_string())
                        .unwrap_or_else(|| "missing".into())
                ),
            });
        }
    }
    for resource in &context.expected_absent_resources {
        let present = match resource {
            ResourceKey::Graph(path) => GraphResourcePath::new(path.as_str())
                .ok()
                .is_some_and(|path| data.graphs.contains_key(&path)),
            ResourceKey::Function(path) => GraphResourcePath::new(path.0.as_ref())
                .ok()
                .and_then(|path| data.graphs.get(&path))
                .is_some_and(|resource| resource.function.is_some()),
            ResourceKey::Database(path) => {
                yss_database_contract::database_id_in_resource_path(&path.0)
                    .is_some_and(|id| data.databases.contains_key(id))
            }
            ResourceKey::Mind(path) => yss_project_model::mind::MindPath::parse(&path.0)
                .ok()
                .is_some_and(|path| data.minds.contains_key(&path)),
            ResourceKey::Doc(path) => yss_project_model::doc::DocPath::parse(&path.0)
                .ok()
                .is_some_and(|path| data.docs.contains_key(&path)),
            ResourceKey::Chart(path) => yss_project_model::chart::ChartPath::parse(&path.0)
                .ok()
                .is_some_and(|path| data.charts.contains_key(&path)),
        };
        if present {
            return Err(ProjectOperationError::ResourceRevisionConflict {
                message: format!("expected {resource:?} to remain absent"),
            });
        }
    }
    Ok(())
}

pub(crate) fn checked_resource_revision(
    resource: impl Into<String>,
    retained: ResourceRevision,
) -> Result<ResourceRevision, ProjectOperationError> {
    retained
        .checked_next()
        .map_err(|error| ProjectOperationError::ResourceRevisionOverflow {
            resource: resource.into(),
            retained: error.retained,
        })
}

pub(super) fn authoritative_function_revision(
    path: &GraphResourcePath,
    incoming: ResourceRevision,
    retained: Option<ResourceRevision>,
) -> Result<ResourceRevision, ProjectOperationError> {
    let Some(retained) = retained else {
        return Ok(incoming);
    };
    let next = checked_resource_revision(path.as_str(), retained)?;
    Ok(std::cmp::max(incoming, next))
}

pub(crate) fn normalize_function_resource_revision(
    path: &GraphResourcePath,
    resource: &mut GraphResourceDocument,
    retained: Option<ResourceRevision>,
) -> Result<ResourceRevision, ProjectOperationError> {
    if resource.kind != yss_graph_document::GraphResourceKind::FunctionGraph {
        return Ok(retained.unwrap_or(ResourceRevision::INITIAL));
    }
    let incoming = resource
        .function
        .as_ref()
        .map(|function| function.revision)
        .unwrap_or(ResourceRevision::INITIAL);
    let revision = authoritative_function_revision(path, incoming, retained)?;
    if let Some(function) = resource.function.as_mut() {
        function.revision = revision;
    }
    Ok(revision)
}

pub(super) fn normalize_function_patch_revisions(
    patch: &mut ProjectDataPatch,
    data: &ProjectData,
    graph_resource_revisions: &std::collections::HashMap<GraphResourcePath, ResourceRevision>,
) -> Result<(), ProjectOperationError> {
    match patch {
        ProjectDataPatch::InsertGraph { path, resource } => {
            normalize_function_resource_revision(
                path,
                resource,
                graph_resource_revisions.get(path).copied(),
            )?;
        }
        ProjectDataPatch::DeclareGraph { path, revision } => {
            if path.kind() == yss_graph_document::GraphResourceKind::FunctionGraph {
                let canonical = authoritative_function_revision(
                    path,
                    *revision,
                    graph_resource_revisions.get(path).copied(),
                )?;
                if canonical != *revision {
                    return Err(ProjectOperationError::ResourceRevisionConflict {
                        message: format!(
                            "declared function '{}' revision changed before publication",
                            path
                        ),
                    });
                }
            }
        }
        ProjectDataPatch::RemoveGraph { path, revision } => {
            if data.graphs.get(path).is_some_and(|resource| {
                resource.kind == yss_graph_document::GraphResourceKind::FunctionGraph
            }) {
                authoritative_function_revision(
                    path,
                    *revision,
                    graph_resource_revisions.get(path).copied(),
                )?;
            }
        }
        ProjectDataPatch::MoveGraph {
            from,
            to,
            moved,
            referenced_graphs,
            ..
        } => {
            if moved.kind == yss_graph_document::GraphResourceKind::FunctionGraph {
                let incoming = moved
                    .function
                    .as_ref()
                    .map(|function| function.revision)
                    .unwrap_or(ResourceRevision::INITIAL);
                authoritative_function_revision(
                    from,
                    incoming,
                    graph_resource_revisions.get(from).copied(),
                )?;
            }
            normalize_function_resource_revision(
                to,
                moved,
                graph_resource_revisions.get(to).copied(),
            )?;
            for (path, resource) in referenced_graphs {
                normalize_function_resource_revision(
                    path,
                    resource,
                    graph_resource_revisions.get(path).copied(),
                )?;
            }
        }
        ProjectDataPatch::Mind(_) | ProjectDataPatch::Doc(_) | ProjectDataPatch::Chart(_) => {}
    }
    Ok(())
}

pub(super) fn canonical_resource_lifecycle_events(
    context: &ProjectTransactionContext,
    patch: &ProjectDataPatch,
    graph_resource_revisions: &std::collections::HashMap<GraphResourcePath, ResourceRevision>,
) -> Result<Vec<yss_project_history::ResourceDeltaEvent>, ProjectOperationError> {
    let graph_key = |path: &GraphResourcePath| ResourceKey::Graph(path.clone());
    let expected_revision = |resource: &ResourceKey| {
        context
            .expected_revisions
            .get(resource)
            .copied()
            .ok_or_else(|| ProjectOperationError::ResourceRevisionConflict {
                message: format!("missing expected revision for {resource:?}"),
            })
    };
    let lifecycle_state =
        |path: &GraphResourcePath, revision| yss_project_history::ResourceLifecycleState {
            revision,
            path: path.as_str().into(),
            kind: match path.kind() {
                yss_graph_document::GraphResourceKind::EventGraph => {
                    yss_project_history::ResourceLifecycleKind::EventGraph
                }
                yss_graph_document::GraphResourceKind::FunctionGraph => {
                    yss_project_history::ResourceLifecycleKind::FunctionGraph
                }
            },
            name: path.display_name().to_string(),
        };
    let lifecycle_delta = |path: &GraphResourcePath, from_revision, to_revision, before, after| {
        yss_project_history::ResourceDeltaEvent {
            resource: graph_key(path),
            from_revision,
            to_revision,
            caused_by: Some(context.operation_id),
            payload: yss_project_history::ResourceDocumentPatch::ResourceLifecycle(
                yss_project_history::ResourceLifecyclePatch { before, after },
            ),
        }
    };
    match patch {
        ProjectDataPatch::InsertGraph { path, resource: _ } => {
            let revision = graph_resource_revisions
                .get(path)
                .copied()
                .map(|retained| checked_resource_revision(path.as_str(), retained))
                .transpose()?
                .unwrap_or(ResourceRevision::INITIAL);
            return Ok(vec![lifecycle_delta(
                path,
                graph_resource_revisions
                    .get(path)
                    .copied()
                    .unwrap_or(revision),
                revision,
                None,
                Some(lifecycle_state(path, revision)),
            )]);
        }
        ProjectDataPatch::DeclareGraph { path, revision } => {
            return Ok(vec![lifecycle_delta(
                path,
                graph_resource_revisions
                    .get(path)
                    .copied()
                    .unwrap_or(*revision),
                *revision,
                None,
                Some(lifecycle_state(path, *revision)),
            )]);
        }
        ProjectDataPatch::RemoveGraph { path, revision } => {
            let revision = *revision;
            return Ok(vec![lifecycle_delta(
                path,
                revision,
                checked_resource_revision(path.as_str(), revision)?,
                Some(lifecycle_state(path, revision)),
                None,
            )]);
        }
        ProjectDataPatch::MoveGraph { .. } => {}
        ProjectDataPatch::Mind(_) | ProjectDataPatch::Doc(_) | ProjectDataPatch::Chart(_) => {
            return Ok(Vec::new());
        }
    }
    let ProjectDataPatch::MoveGraph {
        from,
        to,
        moved: _,
        referenced_graphs,
        ..
    } = patch
    else {
        return Ok(Vec::new());
    };
    let graph_move_patch = || {
        yss_project_history::ResourceDocumentPatch::ResourceMove(
            yss_project_history::ResourcePathMovePatch {
                from: from.as_str().into(),
                to: to.as_str().into(),
            },
        )
    };
    let source_key = graph_key(from);
    let source_revision = expected_revision(&source_key)?;
    let mut deltas = vec![yss_project_history::ResourceDeltaEvent {
        resource: graph_key(to),
        from_revision: source_revision,
        to_revision: checked_resource_revision(
            from.as_str(),
            source_revision.max(
                graph_resource_revisions
                    .get(to)
                    .copied()
                    .unwrap_or(ResourceRevision::INITIAL),
            ),
        )?,
        caused_by: Some(context.operation_id),
        payload: graph_move_patch(),
    }];
    let referenced_graph_deltas = referenced_graphs
        .keys()
        .map(|path| {
            let key = graph_key(path);
            let from_revision = expected_revision(&key)?;
            Ok(yss_project_history::ResourceDeltaEvent {
                from_revision,
                to_revision: checked_resource_revision(path.as_str(), from_revision)?,
                resource: key,
                caused_by: Some(context.operation_id),
                payload: graph_move_patch(),
            })
        })
        .collect::<Result<Vec<_>, ProjectOperationError>>()?;
    deltas.extend(referenced_graph_deltas);
    Ok(deltas)
}

pub(super) fn patch_projection_paths(patch: &ProjectDataPatch, data: &ProjectData) -> Vec<String> {
    let mut paths = std::collections::BTreeSet::new();
    match patch {
        ProjectDataPatch::InsertGraph { .. }
        | ProjectDataPatch::DeclareGraph { .. }
        | ProjectDataPatch::RemoveGraph { .. } => {}
        ProjectDataPatch::MoveGraph {
            from,
            to,
            loaded_referenced_graphs,
            ..
        } => {
            if data.graphs.contains_key(from) {
                paths.insert(to.as_str().to_string());
            }
            paths.extend(
                loaded_referenced_graphs
                    .iter()
                    .map(|path| path.as_str().to_string()),
            );
        }
        ProjectDataPatch::Mind(_) | ProjectDataPatch::Doc(_) | ProjectDataPatch::Chart(_) => {}
    }
    paths.into_iter().collect()
}
