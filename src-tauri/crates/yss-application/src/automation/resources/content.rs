use super::*;
use yss_project_model::mind::{MindEdit, MindNode, MindReference};

pub(in crate::automation) fn inspect_resource(
    application: &ApplicationState,
    session: &ApplicationSession,
    context: &CapabilityInvocationContext,
    request: InspectResourceRequest,
    control: &CapabilityControl,
) -> Result<ResourceInspection> {
    let index = read_index(session)?;
    let info = metadata(&index, &request.resource)?;
    let project = session.project_instance_id().clone();
    let mut version = ResourceVersion {
        revision: info.revision,
        session_id: None,
    };
    let mut dirty = false;
    let content = match request.resource.kind {
        ProjectResourceKind::EventGraph | ProjectResourceKind::FunctionGraph => {
            let AutomationCapabilityResult::GraphInspection(graph) =
                crate::automation::graph::invoke_graph_capability(
                    application,
                    context.clone(),
                    AutomationCapabilityRequest::InspectGraph(InspectGraphRequest {
                        graph_path: request.resource.id.clone(),
                    }),
                    control,
                )?
            else {
                return Err(unavailable());
            };
            let path = graph_path(&request.resource)?;
            let current = session
                .project()
                .read_graph_editing(&project, &path)
                .map_err(project_error)?;
            if current.state.version.revision.get() != graph.revision {
                return Err(conflict());
            }
            version = ResourceVersion {
                revision: graph.revision,
                session_id: Some(current.state.version.session_id.to_string()),
            };
            dirty = current.state.dirty;
            let function = if request.resource.kind == ProjectResourceKind::FunctionGraph {
                let document = session
                    .project()
                    .read_graph_resource_snapshot(&project, &path)
                    .map_err(project_error)?;
                document
                    .function
                    .as_ref()
                    .map(|function| FunctionSignatureInspection {
                        revision: function.revision.get(),
                        parameters: function
                            .signature
                            .parameters
                            .iter()
                            .map(|parameter| FunctionParameterInspection {
                                id: Some(parameter.id.to_string()),
                                name: parameter.name.clone(),
                                type_name: parameter.type_name.clone(),
                            })
                            .collect(),
                        return_type: function.signature.return_type.clone(),
                    })
            } else {
                None
            };
            let after = session
                .project()
                .read_graph_editing(&project, &path)
                .map_err(project_error)?;
            if after.state.version != current.state.version {
                return Err(conflict());
            }
            ResourceContent::Graph {
                graph,
                function,
                can_undo: current.state.can_undo,
                can_redo: current.state.can_redo,
            }
        }
        ProjectResourceKind::Chart => {
            let document = application
                .load_chart_resource(
                    project,
                    chart_path(&request.resource)?,
                    Some(index.publication_revision),
                )
                .map_err(chart_error)?;
            ResourceContent::Chart {
                settings: ChartSettings {
                    database_id: document.database_id,
                    chart_type: match document.chart_type.as_str() {
                        "histogram" => ChartType::Histogram,
                        "scatter" => ChartType::Scatter,
                        "line" => ChartType::Line,
                        _ => return Err(invalid("chartType")),
                    },
                    x: document.encodings.x,
                    y: document.encodings.y,
                },
            }
        }
        ProjectResourceKind::Mind => {
            let snapshot = application
                .read_mind(project, mind_path(&request.resource)?)
                .map_err(file_error)?;
            version = ResourceVersion {
                revision: snapshot.version.revision.get(),
                session_id: Some(snapshot.version.session_id),
            };
            dirty = snapshot.dirty;
            let total_nodes = snapshot.content.nodes.len();
            let nodes = snapshot
                .content
                .nodes
                .into_iter()
                .skip(request.offset)
                .take(request.limit)
                .map(|node| MindTopic {
                    id: node.id,
                    parent_id: node.parent_id,
                    content: node.content,
                    reference: node.reference.map(mind_reference_to_contract),
                })
                .collect::<Vec<_>>();
            let end = request.offset.saturating_add(nodes.len());
            ResourceContent::Mind {
                root_id: snapshot.content.root_id,
                nodes,
                total_nodes,
                next_offset: (end < total_nodes).then_some(end),
            }
        }
        ProjectResourceKind::Doc => {
            let snapshot = application
                .read_doc(project, doc_path(&request.resource)?)
                .map_err(file_error)?;
            version = ResourceVersion {
                revision: snapshot.version.revision.get(),
                session_id: Some(snapshot.version.session_id),
            };
            dirty = snapshot.dirty;
            let total_characters = snapshot.content.0.chars().count();
            let markdown: String = snapshot
                .content
                .0
                .chars()
                .skip(request.offset)
                .take(request.limit)
                .collect();
            let end = request.offset.saturating_add(markdown.chars().count());
            ResourceContent::Doc {
                markdown,
                total_characters,
                next_offset: (end < total_characters).then_some(end),
            }
        }
        ProjectResourceKind::Database => {
            let (content, modified) = database::inspect_database(application, session, &request)?;
            dirty = modified;
            content
        }
    };
    check_version(session, &request.resource, &version)?;
    Ok(ResourceInspection {
        resource: request.resource,
        name: info.display_name,
        version,
        dirty,
        content,
    })
}

fn mind_reference_to_contract(reference: MindReference) -> MindResourceReference {
    match reference {
        MindReference::Resource { path } => MindResourceReference::Resource { path },
        MindReference::GraphNode { path, node_id } => MindResourceReference::GraphNode {
            path: path.as_str().into(),
            node_id,
        },
        MindReference::Database { database_id } => MindResourceReference::Database { database_id },
    }
}
fn mind_reference_from_contract(reference: MindResourceReference) -> Result<MindReference> {
    Ok(match reference {
        MindResourceReference::Resource { path } => MindReference::Resource { path },
        MindResourceReference::GraphNode { path, node_id } => MindReference::GraphNode {
            path: GraphResourcePath::new(&path).map_err(|_| invalid("reference.path"))?,
            node_id,
        },
        MindResourceReference::Database { database_id } => MindReference::Database { database_id },
    })
}

pub(in crate::automation) fn edit_resource(
    application: &ApplicationState,
    session: &ApplicationSession,
    request: EditResourceRequest,
    control: &CapabilityControl,
    publish: Publication<'_>,
) -> Result<ResourceMutationReceipt> {
    check_version(session, &request.resource, &request.version)?;
    let resource = request.resource;
    let project = session.project_instance_id().clone();
    let operation = OperationId::new();
    let mut created_nodes = BTreeMap::new();
    control.check()?;
    let mutation = match request.edit {
        ResourceEdit::Chart { settings } => {
            let mut document = yss_chart_document::ChartDocument::new(settings.database_id);
            document.chart_type = match settings.chart_type {
                ChartType::Histogram => "histogram",
                ChartType::Scatter => "scatter",
                ChartType::Line => "line",
            }
            .into();
            document.encodings = yss_chart_document::ChartEncodings {
                x: settings.x,
                y: settings.y,
            };
            application
                .save_chart_resource(
                    project,
                    operation,
                    chart_path(&resource)?,
                    document,
                    Some(ResourceRevision::new(request.version.revision)),
                )
                .map_err(chart_error)?
        }
        ResourceEdit::Mind { operations } => {
            let mut edits = Vec::with_capacity(operations.len());
            for edit in operations {
                control.check()?;
                let resolve = |id: String, aliases: &BTreeMap<String, String>| -> Result<String> {
                    match id.strip_prefix('$') {
                        Some(alias) => aliases.get(alias).cloned().ok_or_else(|| invalid("nodeId")),
                        None => Ok(id),
                    }
                };
                edits.push(match edit {
                    MindOperation::AddNode {
                        client_id,
                        parent_id,
                        content,
                    } => {
                        if client_id.is_empty()
                            || client_id.len() > 128
                            || created_nodes.contains_key(&client_id)
                        {
                            return Err(invalid("clientId"));
                        }
                        let parent_id = resolve(parent_id, &created_nodes)?;
                        let id = uuid::Uuid::new_v4().to_string();
                        created_nodes.insert(client_id, id.clone());
                        MindEdit::AddNode {
                            node: MindNode {
                                id,
                                parent_id: Some(parent_id),
                                content,
                                reference: None,
                            },
                        }
                    }
                    MindOperation::SetContent { node_id, content } => MindEdit::SetContent {
                        node_id: resolve(node_id, &created_nodes)?,
                        content,
                    },
                    MindOperation::SetReference { node_id, reference } => MindEdit::SetReference {
                        node_id: resolve(node_id, &created_nodes)?,
                        reference: reference.map(mind_reference_from_contract).transpose()?,
                    },
                    MindOperation::MoveNode {
                        node_id,
                        parent_id,
                        before_id,
                    } => MindEdit::MoveNode {
                        node_id: resolve(node_id, &created_nodes)?,
                        parent_id: resolve(parent_id, &created_nodes)?,
                        before_id: before_id
                            .map(|id| resolve(id, &created_nodes))
                            .transpose()?,
                    },
                    MindOperation::RemoveNode { node_id } => MindEdit::RemoveNode {
                        node_id: resolve(node_id, &created_nodes)?,
                    },
                });
            }
            let result = application
                .apply_mind_command(
                    project,
                    operation,
                    FileCommand::Edit {
                        path: mind_path(&resource)?,
                        version: file_version(&request.version)?,
                        edits,
                    },
                )
                .map_err(file_error)?;
            if let Some(snapshot) = result.snapshot {
                created_nodes
                    .retain(|_, id| snapshot.content.nodes.iter().any(|node| &node.id == id));
            }
            result.mutation
        }
        ResourceEdit::Doc { operations } => {
            let path = doc_path(&resource)?;
            let snapshot = application
                .read_doc(project.clone(), path.clone())
                .map_err(file_error)?;
            if snapshot.version != file_version(&request.version)? {
                return Err(conflict());
            }
            let mut markdown = snapshot.content.0;
            for edit in operations {
                control.check()?;
                match edit {
                    MarkdownOperation::SetMarkdown {
                        markdown: replacement,
                    } => markdown = replacement,
                    MarkdownOperation::ReplaceRange {
                        start,
                        end,
                        markdown: replacement,
                    } => {
                        let boundaries: Vec<_> = markdown
                            .char_indices()
                            .map(|(offset, _)| offset)
                            .chain(std::iter::once(markdown.len()))
                            .collect();
                        if start > end || end >= boundaries.len() {
                            return Err(invalid("range"));
                        }
                        markdown.replace_range(boundaries[start]..boundaries[end], &replacement);
                    }
                }
            }
            application
                .apply_doc_command(
                    project,
                    operation,
                    FileCommand::Edit {
                        path,
                        version: snapshot.version,
                        edits: vec![yss_project_model::doc::DocEdit::SetMarkdown { markdown }],
                    },
                )
                .map_err(file_error)?
                .mutation
        }
        ResourceEdit::Database { operation: edit } => {
            application
                .mutate_database_for_application(
                    project,
                    resource.id.clone(),
                    ResourceRevision::new(request.version.revision),
                    operation,
                    database::edit_operation(edit),
                )
                .map_err(database_error)?
                .mutation
        }
        ResourceEdit::FunctionSignature { signature } => {
            let path = graph_path(&resource)?;
            let current = session
                .project()
                .read_graph_resource_snapshot(&project, &path)
                .map_err(project_error)?;
            let current = current.function.ok_or_else(unavailable)?;
            if current.revision.get() != signature.revision {
                return Err(conflict());
            }
            let parameters = signature
                .parameters
                .into_iter()
                .map(|parameter| {
                    let id = match parameter.id {
                        Some(id)
                            if current
                                .signature
                                .parameters
                                .iter()
                                .any(|entry| entry.id.as_str() == id) =>
                        {
                            id
                        }
                        Some(_) => return Err(invalid("parameter.id")),
                        None => uuid::Uuid::new_v4().to_string(),
                    };
                    Ok(yss_project_history::FunctionParameter {
                        id: yss_graph_document::FunctionParameterId::new(id),
                        name: parameter.name,
                        type_name: parameter.type_name,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let after = yss_project_history::FunctionSignature {
                parameters,
                return_type: signature.return_type,
            };
            application
                .update_function_signature(
                    project,
                    path,
                    yss_project_history::MutationRequest::new(
                        ResourceKey::Function(yss_project_history::FunctionResourceKey(
                            resource.id.clone().into_boxed_str(),
                        )),
                        current.revision,
                        operation,
                        yss_project_history::FunctionDocumentPatch::new(current.signature, after),
                    ),
                )
                .map_err(graph_error)?
        }
        ResourceEdit::GraphHistory { redo, graph_hash } => {
            let (request, current) =
                graph_request(application, session, &resource, &request.version)?;
            if crate::automation::graph::graph_hash(&current)? != graph_hash {
                return Err(conflict());
            }
            let result = application
                .change_graph_history(request, redo)
                .map_err(graph_error)?;
            return Ok(graph_receipt(
                resource,
                result.editing.version.revision.get(),
            ));
        }
    };
    let mut receipt = committed(mutation, publish)?;
    receipt.created_nodes = created_nodes;
    Ok(receipt)
}

fn graph_request(
    application: &ApplicationState,
    session: &ApplicationSession,
    resource: &ProjectResourceRef,
    version: &ResourceVersion,
) -> Result<(
    crate::graph::editing::GraphEditRequest,
    std::sync::Arc<yss_graph_document::GraphDocument>,
)> {
    let path = graph_path(resource)?;
    application
        .open_graph(crate::graph::open::OpenGraphRequest::new(
            session.project_instance_id().clone(),
            path.clone(),
            0,
            "en-US",
        ))
        .map_err(|_| unavailable())?;
    let current = session
        .project()
        .read_graph_editing(session.project_instance_id(), &path)
        .map_err(project_error)?;
    if current.state.version.revision.get() != version.revision
        || version
            .session_id
            .as_ref()
            .is_some_and(|id| id != &current.state.version.session_id.to_string())
    {
        return Err(conflict());
    }
    Ok((
        crate::graph::editing::GraphEditRequest {
            project_instance_id: session.project_instance_id().clone(),
            graph_path: path,
            version: current.state.version,
            operation_id: OperationId::new(),
            locale: "en-US".into(),
        },
        current.document,
    ))
}
fn graph_receipt(resource: ProjectResourceRef, revision: u64) -> ResourceMutationReceipt {
    ResourceMutationReceipt {
        publication_revision: None,
        changes: vec![ResourceChange {
            resource,
            revision,
            revision_kind: ResourceRevisionKind::Resource,
            deleted: false,
        }],
        moves: vec![],
        created_nodes: BTreeMap::new(),
    }
}
pub(super) fn save_graph(
    application: &ApplicationState,
    session: &ApplicationSession,
    resource: &ProjectResourceRef,
    version: &ResourceVersion,
) -> Result<ResourceMutationReceipt> {
    let (request, _) = graph_request(application, session, resource, version)?;
    let result = application
        .save_current_graph(request)
        .map_err(graph_error)?;
    Ok(graph_receipt(
        resource.clone(),
        result.resource_revision.get(),
    ))
}
