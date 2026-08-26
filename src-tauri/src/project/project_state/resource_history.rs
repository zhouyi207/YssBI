use super::*;

pub(super) fn graph_document_references_path(
    document: &crate::graph_document::GraphDocument,
    target: &str,
) -> bool {
    document.nodes.values().any(|node| {
        node.parameters.values().any(|value| {
            value.as_str().is_some_and(|path| {
                crate::project::graph_resource_index::normalize_resource_path(path) == target
            })
        })
    })
}

pub(super) fn variable_scope_references_path(
    scope: &crate::variable::VariableScope,
    target: &str,
) -> bool {
    match scope {
        crate::variable::VariableScope::Global => false,
        crate::variable::VariableScope::Event { event_path } => {
            crate::project::graph_resource_index::normalize_resource_path(event_path) == target
        }
        crate::variable::VariableScope::Function { function_path } => {
            crate::project::graph_resource_index::normalize_resource_path(function_path) == target
        }
    }
}

pub(in crate::project) fn validate_context_revisions(
    context: &ProjectTransactionContext,
    data: &ProjectData,
    graph_revisions: &std::collections::HashMap<
        GraphResourcePath,
        crate::graph_document::GraphRevision,
    >,
    variable_revisions: &std::collections::HashMap<
        crate::variable::VariableId,
        VariableRevisionEntry,
    >,
    worksheet_revisions: &std::collections::HashMap<
        WorksheetResourcePath,
        crate::project::ResourceRevision,
    >,
) -> Result<(), ProjectFilesystemError> {
    for resource in &context.affected_resources {
        let expected = context.expected_revisions.get(resource).ok_or_else(|| {
            ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("missing expected revision for {resource:?}"),
            }
        })?;
        let actual = match resource {
            ResourceKey::Graph(path) => {
                GraphResourcePath::new(path.as_str()).ok().and_then(|path| {
                    data.graphs
                        .get(&path)
                        .map(|resource| {
                            ResourceRevision::from_graph_revision(resource.document.revision)
                        })
                        .or_else(|| {
                            graph_revisions
                                .get(&path)
                                .copied()
                                .map(ResourceRevision::from_graph_revision)
                        })
                })
            }
            ResourceKey::Function(path) => GraphResourcePath::new(path.0.as_ref())
                .ok()
                .and_then(|path| data.graphs.get(&path))
                .and_then(|resource| resource.function.as_ref())
                .map(|function| function.revision),
            ResourceKey::Variable(path) => path
                .0
                .strip_prefix("variables/")
                .or(Some(path.0.as_ref()))
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
                .map(crate::variable::VariableId::from)
                .and_then(|id| variable_revisions.get(&id).map(|entry| entry.revision)),
            ResourceKey::Database(_) => None,
            ResourceKey::Worksheet(path) => WorksheetResourcePath::parse(path.0.as_ref())
                .ok()
                .and_then(|path| worksheet_revisions.get(&path).copied()),
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
            ResourceKey::Variable(path) => path
                .0
                .strip_prefix("variables/")
                .or(Some(path.0.as_ref()))
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
                .map(crate::variable::VariableId::from)
                .is_some_and(|id| data.variables.contains_key(&id)),
            ResourceKey::Database(path) => path
                .0
                .strip_prefix("databases/")
                .is_some_and(|id| data.databases.contains_key(id)),
            ResourceKey::Worksheet(path) => WorksheetResourcePath::parse(path.0.as_ref())
                .ok()
                .is_some_and(|path| data.worksheets.contains_key(&path)),
        };
        if present {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("expected {resource:?} to remain absent"),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_worksheet_path_insertion(
    data: &ProjectData,
    worksheet_path: &WorksheetResourcePath,
) -> Result<(), ProjectFilesystemError> {
    let portable_key = worksheet_path.display_name().portable_key();
    if data.worksheets.keys().any(|existing| {
        existing != worksheet_path && existing.display_name().portable_key() == portable_key
    }) {
        return Err(ProjectFilesystemError::ResourceRevisionConflict {
            message: format!(
                "worksheet path '{}' conflicts with an existing portable name",
                worksheet_path.as_str()
            ),
        });
    }
    Ok(())
}

pub(in crate::project) fn checked_resource_revision(
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

pub(in crate::project::project_state) fn checked_graph_revision(
    resource: &str,
    retained: crate::graph_document::GraphRevision,
) -> Result<crate::graph_document::GraphRevision, ProjectFilesystemError> {
    retained
        .checked_next()
        .map_err(|error| ProjectFilesystemError::ResourceRevisionOverflow {
            resource: resource.into(),
            retained: error.retained,
        })
}

pub(super) fn authoritative_function_revision(
    path: &GraphResourcePath,
    incoming: crate::graph_document::GraphRevision,
    retained: Option<crate::graph_document::GraphRevision>,
) -> Result<crate::graph_document::GraphRevision, ProjectFilesystemError> {
    let Some(retained) = retained else {
        return Ok(incoming);
    };
    let next = checked_graph_revision(path.as_str(), retained)?;
    Ok(std::cmp::max(incoming, next))
}

pub(super) fn normalize_loaded_function_resource_revision(
    path: &GraphResourcePath,
    resource: &mut GraphResourceDocument,
    retained: Option<crate::graph_document::GraphRevision>,
) -> Result<crate::graph_document::GraphRevision, ProjectFilesystemError> {
    if resource.kind != crate::project::GraphDocumentKind::Function {
        return Ok(resource.document.revision);
    }
    let incoming = resource.document.revision;
    let revision = match retained {
        Some(retained) if incoming < retained => {
            authoritative_function_revision(path, incoming, Some(retained))?
        }
        _ => incoming,
    };
    resource.document.revision = revision;
    if let Some(function) = resource.function.as_mut() {
        function.revision = ResourceRevision::from_graph_revision(revision);
    }
    Ok(revision)
}

pub(in crate::project) fn normalize_function_resource_revision(
    path: &GraphResourcePath,
    resource: &mut GraphResourceDocument,
    retained: Option<crate::graph_document::GraphRevision>,
) -> Result<crate::graph_document::GraphRevision, ProjectFilesystemError> {
    if resource.kind != crate::project::GraphDocumentKind::Function {
        return Ok(resource.document.revision);
    }
    let revision = authoritative_function_revision(path, resource.document.revision, retained)?;
    resource.document.revision = revision;
    if let Some(function) = resource.function.as_mut() {
        function.revision = ResourceRevision::from_graph_revision(revision);
    }
    Ok(revision)
}

pub(super) fn normalize_function_patch_revisions(
    patch: &mut ResourceDocumentPatch,
    data: &ProjectData,
    graph_revisions: &std::collections::HashMap<
        GraphResourcePath,
        crate::graph_document::GraphRevision,
    >,
) -> Result<(), ProjectFilesystemError> {
    match patch {
        ResourceDocumentPatch::InsertGraph { path, resource } => {
            normalize_function_resource_revision(
                path,
                resource,
                graph_revisions.get(path).copied(),
            )?;
        }
        ResourceDocumentPatch::DeclareGraph { path, revision } => {
            if path.kind() == crate::graph_document::GraphResourceKind::Function {
                let canonical = authoritative_function_revision(
                    path,
                    revision.to_graph_revision(),
                    graph_revisions.get(path).copied(),
                )?;
                if canonical != revision.to_graph_revision() {
                    return Err(ProjectFilesystemError::ResourceRevisionConflict {
                        message: format!(
                            "declared function '{}' revision changed before publication",
                            path
                        ),
                    });
                }
            }
        }
        ResourceDocumentPatch::RemoveGraph { path, revision } => {
            if data.graphs.get(path).is_some_and(|resource| {
                resource.kind == crate::project::GraphDocumentKind::Function
            }) {
                authoritative_function_revision(
                    path,
                    revision.to_graph_revision(),
                    graph_revisions.get(path).copied(),
                )?;
            }
        }
        ResourceDocumentPatch::MoveGraph {
            from,
            to,
            moved,
            referenced_graphs,
            ..
        } => {
            if moved.kind == crate::project::GraphDocumentKind::Function {
                authoritative_function_revision(
                    from,
                    moved.document.revision,
                    graph_revisions.get(from).copied(),
                )?;
            }
            normalize_function_resource_revision(to, moved, graph_revisions.get(to).copied())?;
            for (path, resource) in referenced_graphs {
                normalize_function_resource_revision(
                    path,
                    resource,
                    graph_revisions.get(path).copied(),
                )?;
            }
        }
        ResourceDocumentPatch::UnloadGraph { .. }
        | ResourceDocumentPatch::PatchVariables { .. }
        | ResourceDocumentPatch::UpsertWorksheet { .. }
        | ResourceDocumentPatch::RemoveWorksheet { .. }
        | ResourceDocumentPatch::MoveWorksheet { .. } => {}
    }
    Ok(())
}

pub(super) fn worksheet_document_state(
    document: &crate::project::WorksheetDocument,
) -> crate::node_system::document::WorksheetDocumentState {
    crate::node_system::document::WorksheetDocumentState {
        database_id: document.database_id.clone(),
        chart_type: document.chart_type.clone(),
        encodings: document.encodings.clone(),
    }
}

pub(super) fn worksheet_lifecycle_state(
    path: &WorksheetResourcePath,
    revision: ResourceRevision,
) -> crate::node_system::document::ResourceLifecycleState {
    crate::node_system::document::ResourceLifecycleState {
        revision,
        path: path.as_str().into(),
        kind: crate::node_system::document::ResourceLifecycleKind::Worksheet,
        name: path.display_name().as_str().to_string(),
    }
}

pub(super) fn worksheet_history_publication(
    operation_id: OperationId,
    patch: &ResourceDocumentPatch,
    data: &ProjectData,
    revisions: &std::collections::HashMap<WorksheetResourcePath, ResourceRevision>,
) -> Result<
    (
        Vec<crate::node_system::document::ResourceDeltaEvent>,
        Option<ProjectHistoryTransaction>,
    ),
    ProjectFilesystemError,
> {
    let worksheet_key = |path: &WorksheetResourcePath| {
        ResourceKey::Worksheet(crate::node_system::document::WorksheetResourceKey(
            path.as_str().into(),
        ))
    };
    match patch {
        ResourceDocumentPatch::UpsertWorksheet { path, document } => {
            let before = data.worksheets.get(path);
            let retained = revisions.get(path).copied();
            let revision = match retained {
                Some(retained) => checked_resource_revision(path.as_str(), retained)?,
                None => ResourceRevision::INITIAL,
            };
            let mut after = document.clone();
            after.revision = revision;
            let (from_revision, payload, transaction) = if let Some(before) = before {
                let forward = crate::node_system::document::WorksheetDocumentPatch {
                    before: worksheet_document_state(before),
                    after: worksheet_document_state(&after),
                };
                (
                    before.revision,
                    crate::node_system::document::ResourceDocumentPatch::Worksheet(forward.clone()),
                    ProjectHistoryTransaction::new(
                        operation_id,
                        vec![crate::node_system::document::ResourcePatch::worksheet(
                            crate::node_system::document::WorksheetResourceKey(
                                path.as_str().into(),
                            ),
                            before.revision,
                            forward,
                        )],
                    ),
                )
            } else {
                let forward = crate::node_system::document::ResourceLifecyclePatch {
                    before: None,
                    after: Some(worksheet_lifecycle_state(path, revision)),
                };
                (
                    retained.unwrap_or(revision),
                    crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(
                        forward.clone(),
                    ),
                    ProjectHistoryTransaction::resource_lifecycle(
                        operation_id,
                        forward,
                        crate::node_system::document::ResourceLifecycleHistoryPayload::Worksheet {
                            document: after.clone(),
                        },
                    ),
                )
            };
            Ok((
                vec![crate::node_system::document::ResourceDeltaEvent {
                    resource: worksheet_key(path),
                    from_revision,
                    to_revision: revision,
                    caused_by: Some(operation_id),
                    payload,
                }],
                Some(transaction),
            ))
        }
        ResourceDocumentPatch::RemoveWorksheet { path, revision } => {
            let document = data.worksheets.get(path).cloned().ok_or_else(|| {
                ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!("worksheet '{}' is absent", path.as_str()),
                }
            })?;
            let forward = crate::node_system::document::ResourceLifecyclePatch {
                before: Some(worksheet_lifecycle_state(path, *revision)),
                after: None,
            };
            Ok((
                vec![crate::node_system::document::ResourceDeltaEvent {
                    resource: worksheet_key(path),
                    from_revision: *revision,
                    to_revision: checked_resource_revision(path.as_str(), *revision)?,
                    caused_by: Some(operation_id),
                    payload: crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(
                        forward.clone(),
                    ),
                }],
                Some(ProjectHistoryTransaction::resource_lifecycle(
                    operation_id,
                    forward,
                    crate::node_system::document::ResourceLifecycleHistoryPayload::Worksheet {
                        document,
                    },
                )),
            ))
        }
        ResourceDocumentPatch::MoveWorksheet { from, to, moved } => Ok((
            vec![crate::node_system::document::ResourceDeltaEvent {
                resource: worksheet_key(to),
                from_revision: revisions.get(from).copied().unwrap_or(moved.revision),
                to_revision: moved.revision,
                caused_by: Some(operation_id),
                payload: crate::node_system::document::ResourceDocumentPatch::ResourceMove(
                    crate::node_system::document::ResourcePathMovePatch {
                        from: from.as_str().into(),
                        to: to.as_str().into(),
                    },
                ),
            }],
            Some(ProjectHistoryTransaction::worksheet_resource_move(
                operation_id,
                from.as_str(),
                to.as_str(),
                moved.clone(),
            )),
        )),
        _ => Ok((Vec::new(), None)),
    }
}

pub(super) fn canonical_resource_lifecycle_events(
    context: &ProjectTransactionContext,
    patch: &ResourceDocumentPatch,
    graph_revisions: &std::collections::HashMap<
        GraphResourcePath,
        crate::graph_document::GraphRevision,
    >,
) -> Result<Vec<crate::node_system::document::ResourceDeltaEvent>, ProjectFilesystemError> {
    let graph_key = |path: &GraphResourcePath| ResourceKey::Graph(path.clone());
    let lifecycle_state =
        |path: &GraphResourcePath, revision| crate::node_system::document::ResourceLifecycleState {
            revision,
            path: path.as_str().into(),
            kind: match path.kind() {
                crate::graph_document::GraphResourceKind::Event => {
                    crate::node_system::document::ResourceLifecycleKind::Event
                }
                crate::graph_document::GraphResourceKind::Function => {
                    crate::node_system::document::ResourceLifecycleKind::Function
                }
            },
            name: path.display_name().to_string(),
        };
    let lifecycle_delta = |path: &GraphResourcePath, from_revision, to_revision, before, after| {
        crate::node_system::document::ResourceDeltaEvent {
            resource: graph_key(path),
            from_revision,
            to_revision,
            caused_by: Some(context.operation_id),
            payload: crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(
                crate::node_system::document::ResourceLifecyclePatch { before, after },
            ),
        }
    };
    match patch {
        ResourceDocumentPatch::InsertGraph { path, resource } => {
            let revision = ResourceRevision::from_graph_revision(resource.document.revision);
            return Ok(vec![lifecycle_delta(
                path,
                graph_revisions
                    .get(path)
                    .copied()
                    .map(ResourceRevision::from_graph_revision)
                    .unwrap_or(revision),
                revision,
                None,
                Some(lifecycle_state(path, revision)),
            )]);
        }
        ResourceDocumentPatch::DeclareGraph { path, revision } => {
            return Ok(vec![lifecycle_delta(
                path,
                graph_revisions
                    .get(path)
                    .copied()
                    .map(ResourceRevision::from_graph_revision)
                    .unwrap_or(*revision),
                *revision,
                None,
                Some(lifecycle_state(path, *revision)),
            )]);
        }
        ResourceDocumentPatch::UnloadGraph { path } => {
            let revision = context.expected_revisions[&graph_key(path)];
            return Ok(vec![lifecycle_delta(
                path,
                revision,
                revision,
                None,
                Some(lifecycle_state(path, revision)),
            )]);
        }
        ResourceDocumentPatch::RemoveGraph { path, revision } => {
            let revision = *revision;
            return Ok(vec![lifecycle_delta(
                path,
                revision,
                checked_resource_revision(path.as_str(), revision)?,
                Some(lifecycle_state(path, revision)),
                None,
            )]);
        }
        ResourceDocumentPatch::MoveGraph { .. } => {}
        ResourceDocumentPatch::PatchVariables { .. }
        | ResourceDocumentPatch::UpsertWorksheet { .. }
        | ResourceDocumentPatch::RemoveWorksheet { .. }
        | ResourceDocumentPatch::MoveWorksheet { .. } => return Ok(Vec::new()),
    }
    let ResourceDocumentPatch::MoveGraph {
        from,
        to,
        moved,
        referenced_graphs,
        referenced_variables,
        ..
    } = patch
    else {
        unreachable!("non-move graph lifecycle patches returned above")
    };
    let graph_move_patch = || {
        crate::node_system::document::ResourceDocumentPatch::ResourceMove(
            crate::node_system::document::ResourcePathMovePatch {
                from: from.as_str().into(),
                to: to.as_str().into(),
            },
        )
    };
    let source_key = graph_key(from);
    let mut deltas = vec![crate::node_system::document::ResourceDeltaEvent {
        resource: graph_key(to),
        from_revision: context.expected_revisions[&source_key],
        to_revision: ResourceRevision::from_graph_revision(moved.document.revision),
        caused_by: Some(context.operation_id),
        payload: graph_move_patch(),
    }];
    deltas.extend(referenced_graphs.iter().map(|(path, resource)| {
        let key = graph_key(path);
        crate::node_system::document::ResourceDeltaEvent {
            from_revision: context.expected_revisions[&key],
            to_revision: ResourceRevision::from_graph_revision(resource.document.revision),
            resource: key,
            caused_by: Some(context.operation_id),
            payload: graph_move_patch(),
        }
    }));
    let variable_deltas = referenced_variables
        .keys()
        .map(|id| {
            let resource_path = format!("variables/{id}");
            let key = ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
                resource_path.clone().into(),
            ));
            let from_revision = context.expected_revisions[&key];
            Ok(crate::node_system::document::ResourceDeltaEvent {
                resource: key,
                from_revision,
                to_revision: checked_resource_revision(resource_path, from_revision)?,
                caused_by: Some(context.operation_id),
                payload: crate::node_system::document::ResourceDocumentPatch::VariableScopeMove(
                    crate::node_system::document::ResourcePathMovePatch {
                        from: from.as_str().into(),
                        to: to.as_str().into(),
                    },
                ),
            })
        })
        .collect::<Result<Vec<_>, ProjectFilesystemError>>()?;
    deltas.extend(variable_deltas);
    Ok(deltas)
}

pub(super) fn patch_projection_paths(
    patch: &ResourceDocumentPatch,
    data: &ProjectData,
) -> Vec<String> {
    let mut paths = std::collections::BTreeSet::new();
    match patch {
        ResourceDocumentPatch::InsertGraph { path, .. }
        | ResourceDocumentPatch::DeclareGraph { path, .. }
        | ResourceDocumentPatch::RemoveGraph { path, .. }
        | ResourceDocumentPatch::UnloadGraph { path } => {
            paths.insert(path.as_str().to_string());
        }
        ResourceDocumentPatch::MoveGraph {
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
        ResourceDocumentPatch::PatchVariables { .. }
        | ResourceDocumentPatch::UpsertWorksheet { .. }
        | ResourceDocumentPatch::RemoveWorksheet { .. }
        | ResourceDocumentPatch::MoveWorksheet { .. } => {}
    }
    paths.into_iter().collect()
}

pub(super) fn compile_product_invalidation_for_resource_patch(
    patch: &ResourceDocumentPatch,
) -> super::CompileProductInvalidation {
    match patch {
        ResourceDocumentPatch::InsertGraph { path, .. }
        | ResourceDocumentPatch::DeclareGraph { path, .. } => {
            super::CompileProductInvalidation::Graphs(vec![path.clone()])
        }
        ResourceDocumentPatch::RemoveGraph { path, .. }
        | ResourceDocumentPatch::UnloadGraph { path } => {
            super::CompileProductInvalidation::Graphs(vec![path.clone()])
        }
        ResourceDocumentPatch::MoveGraph {
            from,
            to,
            referenced_graphs,
            ..
        } => {
            let mut paths = vec![from.clone(), to.clone()];
            paths.extend(referenced_graphs.keys().cloned());
            super::CompileProductInvalidation::Graphs(paths)
        }
        ResourceDocumentPatch::PatchVariables { .. } => super::CompileProductInvalidation::None,
        ResourceDocumentPatch::UpsertWorksheet { .. }
        | ResourceDocumentPatch::RemoveWorksheet { .. }
        | ResourceDocumentPatch::MoveWorksheet { .. } => super::CompileProductInvalidation::None,
    }
}

pub(super) fn validate_graph_resource(
    path: &GraphResourcePath,
    resource: &GraphResourceDocument,
) -> Result<(), ProjectFilesystemError> {
    resource
        .validate()
        .map_err(|source| ProjectFilesystemError::InvalidGraphDocument {
            path: path.clone(),
            source,
        })
}

pub(super) fn preflight_resource_patch_graphs(
    patch: &ResourceDocumentPatch,
) -> Result<(), ProjectFilesystemError> {
    match patch {
        ResourceDocumentPatch::InsertGraph { path, resource } => {
            validate_graph_resource(path, resource)?;
        }
        ResourceDocumentPatch::MoveGraph {
            to,
            moved,
            referenced_graphs,
            ..
        } => {
            validate_graph_resource(to, moved)?;
            for (path, resource) in referenced_graphs {
                validate_graph_resource(path, resource)?;
            }
        }
        ResourceDocumentPatch::DeclareGraph { .. }
        | ResourceDocumentPatch::RemoveGraph { .. }
        | ResourceDocumentPatch::UnloadGraph { .. }
        | ResourceDocumentPatch::PatchVariables { .. }
        | ResourceDocumentPatch::UpsertWorksheet { .. }
        | ResourceDocumentPatch::RemoveWorksheet { .. }
        | ResourceDocumentPatch::MoveWorksheet { .. } => {}
    }
    Ok(())
}

pub(super) fn affected_projection_paths(
    deltas: &[crate::node_system::document::ResourceDeltaEvent],
    data: &ProjectData,
) -> Vec<String> {
    let changed_functions = deltas
        .iter()
        .filter_map(|delta| match &delta.resource {
            crate::node_system::document::ResourceKey::Function(path) => Some(path.0.to_string()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut paths = deltas
        .iter()
        .filter_map(|delta| match &delta.resource {
            crate::node_system::document::ResourceKey::Graph(path) => {
                Some(path.as_str().to_owned())
            }
            crate::node_system::document::ResourceKey::Function(path) => Some(path.0.to_string()),
            crate::node_system::document::ResourceKey::Variable(_)
            | crate::node_system::document::ResourceKey::Database(_)
            | crate::node_system::document::ResourceKey::Worksheet(_) => None,
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
