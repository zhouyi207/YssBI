use super::*;
use yss_project_model::ProjectDataPatch;

pub(crate) fn validate_context_revisions(
    context: &ProjectTransactionContext,
    data: &ProjectData,
    graph_resource_revisions: &std::collections::HashMap<GraphResourcePath, ResourceRevision>,
    chart_revisions: &std::collections::HashMap<
        ChartResourcePath,
        yss_project_identity::ResourceRevision,
    >,
) -> Result<(), ProjectFilesystemError> {
    for resource in &context.affected_resources {
        let expected = context.expected_revisions.get(resource).ok_or_else(|| {
            ProjectFilesystemError::ResourceRevisionConflict {
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
            ResourceKey::Chart(path) => ChartResourcePath::parse(path.0.as_ref())
                .ok()
                .and_then(|path| chart_revisions.get(&path).copied()),
        };
        if actual != Some(*expected) {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
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
            ResourceKey::Database(path) => path
                .0
                .strip_prefix("databases/")
                .is_some_and(|id| data.databases.contains_key(id)),
            ResourceKey::Chart(path) => ChartResourcePath::parse(path.0.as_ref())
                .ok()
                .is_some_and(|path| data.charts.contains_key(&path)),
        };
        if present {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("expected {resource:?} to remain absent"),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_chart_path_insertion(
    data: &ProjectData,
    chart_path: &ChartResourcePath,
) -> Result<(), ProjectFilesystemError> {
    let portable_key = chart_path.display_name().portable_key();
    if data.charts.keys().any(|existing| {
        existing != chart_path && existing.display_name().portable_key() == portable_key
    }) {
        return Err(ProjectFilesystemError::ResourceRevisionConflict {
            message: format!(
                "chart path '{}' conflicts with an existing portable name",
                chart_path.as_str()
            ),
        });
    }
    Ok(())
}

pub(crate) fn checked_resource_revision(
    resource: impl Into<String>,
    retained: ResourceRevision,
) -> Result<ResourceRevision, ProjectFilesystemError> {
    retained
        .checked_next()
        .map_err(|error| ProjectFilesystemError::ResourceRevisionOverflow {
            resource: resource.into(),
            retained: error.retained,
        })
}

pub(super) fn authoritative_function_revision(
    path: &GraphResourcePath,
    incoming: ResourceRevision,
    retained: Option<ResourceRevision>,
) -> Result<ResourceRevision, ProjectFilesystemError> {
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
) -> Result<ResourceRevision, ProjectFilesystemError> {
    if resource.kind != yss_graph_document::GraphResourceKind::Function {
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
) -> Result<(), ProjectFilesystemError> {
    match patch {
        ProjectDataPatch::InsertGraph { path, resource } => {
            normalize_function_resource_revision(
                path,
                resource,
                graph_resource_revisions.get(path).copied(),
            )?;
        }
        ProjectDataPatch::DeclareGraph { path, revision } => {
            if path.kind() == yss_graph_document::GraphResourceKind::Function {
                let canonical = authoritative_function_revision(
                    path,
                    *revision,
                    graph_resource_revisions.get(path).copied(),
                )?;
                if canonical != *revision {
                    return Err(ProjectFilesystemError::ResourceRevisionConflict {
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
                resource.kind == yss_graph_document::GraphResourceKind::Function
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
            if moved.kind == yss_graph_document::GraphResourceKind::Function {
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
        ProjectDataPatch::UpsertChart { .. }
        | ProjectDataPatch::RemoveChart { .. }
        | ProjectDataPatch::MoveChart { .. } => {}
    }
    Ok(())
}

pub(super) fn chart_document_state(
    document: &ChartDocument,
) -> yss_project_history::ChartDocumentState {
    yss_project_history::ChartDocumentState {
        database_id: document.database_id.clone(),
        chart_type: document.chart_type.clone(),
        encodings: document.encodings.clone(),
    }
}

pub(super) fn chart_lifecycle_state(
    path: &ChartResourcePath,
    revision: ResourceRevision,
) -> yss_project_history::ResourceLifecycleState {
    yss_project_history::ResourceLifecycleState {
        revision,
        path: path.as_str().into(),
        kind: yss_project_history::ResourceLifecycleKind::Chart,
        name: path.display_name().as_str().to_string(),
    }
}

pub(super) fn chart_publication_deltas(
    operation_id: OperationId,
    patch: &ProjectDataPatch,
    data: &ProjectData,
    revisions: &std::collections::HashMap<ChartResourcePath, ResourceRevision>,
) -> Result<Vec<yss_project_history::ResourceDeltaEvent>, ProjectFilesystemError> {
    let chart_key = |path: &ChartResourcePath| {
        ResourceKey::Chart(yss_project_history::ChartResourceKey(path.as_str().into()))
    };
    match patch {
        ProjectDataPatch::UpsertChart { path, document } => {
            let before = data.charts.get(path);
            let retained = revisions.get(path).copied();
            let revision = match retained {
                Some(retained) => checked_resource_revision(path.as_str(), retained)?,
                None => ResourceRevision::INITIAL,
            };
            let (from_revision, payload) = if let Some(before) = before {
                (
                    retained.ok_or_else(|| ProjectFilesystemError::ResourceRevisionConflict {
                        message: format!("chart '{}' has no resource revision", path.as_str()),
                    })?,
                    yss_project_history::ResourceDocumentPatch::Chart(
                        yss_project_history::ChartDocumentPatch {
                            before: chart_document_state(before),
                            after: chart_document_state(document),
                        },
                    ),
                )
            } else {
                (
                    retained.unwrap_or(revision),
                    yss_project_history::ResourceDocumentPatch::ResourceLifecycle(
                        yss_project_history::ResourceLifecyclePatch {
                            before: None,
                            after: Some(chart_lifecycle_state(path, revision)),
                        },
                    ),
                )
            };
            Ok(vec![yss_project_history::ResourceDeltaEvent {
                resource: chart_key(path),
                from_revision,
                to_revision: revision,
                caused_by: Some(operation_id),
                payload,
            }])
        }
        ProjectDataPatch::RemoveChart { path, revision } => {
            if !data.charts.contains_key(path) {
                return Err(ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!("chart '{}' is absent", path.as_str()),
                });
            }
            Ok(vec![yss_project_history::ResourceDeltaEvent {
                resource: chart_key(path),
                from_revision: *revision,
                to_revision: checked_resource_revision(path.as_str(), *revision)?,
                caused_by: Some(operation_id),
                payload: yss_project_history::ResourceDocumentPatch::ResourceLifecycle(
                    yss_project_history::ResourceLifecyclePatch {
                        before: Some(chart_lifecycle_state(path, *revision)),
                        after: None,
                    },
                ),
            }])
        }
        ProjectDataPatch::MoveChart { from, to, .. } => {
            let from_revision = revisions.get(from).copied().ok_or_else(|| {
                ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!("chart '{}' has no resource revision", from.as_str()),
                }
            })?;
            Ok(vec![yss_project_history::ResourceDeltaEvent {
                resource: chart_key(to),
                from_revision,
                to_revision: checked_resource_revision(from.as_str(), from_revision)?,
                caused_by: Some(operation_id),
                payload: yss_project_history::ResourceDocumentPatch::ResourceMove(
                    yss_project_history::ResourcePathMovePatch {
                        from: from.as_str().into(),
                        to: to.as_str().into(),
                    },
                ),
            }])
        }
        _ => Ok(Vec::new()),
    }
}

pub(super) fn canonical_resource_lifecycle_events(
    context: &ProjectTransactionContext,
    patch: &ProjectDataPatch,
    graph_resource_revisions: &std::collections::HashMap<GraphResourcePath, ResourceRevision>,
) -> Result<Vec<yss_project_history::ResourceDeltaEvent>, ProjectFilesystemError> {
    let graph_key = |path: &GraphResourcePath| ResourceKey::Graph(path.clone());
    let expected_revision = |resource: &ResourceKey| {
        context
            .expected_revisions
            .get(resource)
            .copied()
            .ok_or_else(|| ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("missing expected revision for {resource:?}"),
            })
    };
    let lifecycle_state =
        |path: &GraphResourcePath, revision| yss_project_history::ResourceLifecycleState {
            revision,
            path: path.as_str().into(),
            kind: match path.kind() {
                yss_graph_document::GraphResourceKind::Event => {
                    yss_project_history::ResourceLifecycleKind::Event
                }
                yss_graph_document::GraphResourceKind::Function => {
                    yss_project_history::ResourceLifecycleKind::Function
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
        ProjectDataPatch::UpsertChart { .. }
        | ProjectDataPatch::RemoveChart { .. }
        | ProjectDataPatch::MoveChart { .. } => return Ok(Vec::new()),
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
        .collect::<Result<Vec<_>, ProjectFilesystemError>>()?;
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
        ProjectDataPatch::UpsertChart { .. }
        | ProjectDataPatch::RemoveChart { .. }
        | ProjectDataPatch::MoveChart { .. } => {}
    }
    paths.into_iter().collect()
}

pub(super) fn affected_projection_paths(
    deltas: &[yss_project_history::ResourceDeltaEvent],
    data: &ProjectData,
) -> Vec<String> {
    let changed_functions = deltas
        .iter()
        .filter_map(|delta| match &delta.resource {
            yss_project_history::ResourceKey::Function(path) => Some(path.0.to_string()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut paths = deltas
        .iter()
        .filter_map(|delta| match &delta.resource {
            yss_project_history::ResourceKey::Graph(path) => Some(path.as_str().to_owned()),
            yss_project_history::ResourceKey::Function(path) => Some(path.0.to_string()),
            yss_project_history::ResourceKey::Database(_)
            | yss_project_history::ResourceKey::Chart(_) => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    if !changed_functions.is_empty() {
        for (graph_path, graph) in &data.graphs {
            let calls_changed_function = graph.document.nodes.values().any(|node| {
                node.node_type.as_str() == "yssbi.project.function.call"
                    && node.parameters.iter().any(|(key, value)| {
                        key.as_str() == "target"
                            && value
                                .as_str()
                                .is_some_and(|target| changed_functions.contains(target))
                    })
            });
            if calls_changed_function {
                paths.insert(graph_path.as_str().to_string());
            }
        }
    }
    paths.into_iter().collect()
}
