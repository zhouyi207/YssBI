use super::*;

pub(in crate::automation) fn inspect_resource(
    application: &ApplicationState,
    session: &ApplicationSession,
    request: InspectResourceRequest,
    control: &CapabilityControl,
) -> Result<ResourceInspection> {
    let index = read_index(session)?;
    let info = index
        .resources
        .iter()
        .find(|entry| entry.resource == request.resource)
        .ok_or_else(unavailable)?;
    let project = session.project_instance_id().clone();
    let mut version = ResourceVersion {
        revision: info.revision.get(),
        session_id: info.session_id.clone(),
    };
    let mut dirty = info.dirty.unwrap_or(false);
    let content = match request.resource.kind {
        ProjectResourceKind::FunctionGraph => {
            // Function signatures retain their existing capability; no graph body is projected.
            let path = graph_path(&request.resource)?;
            session
                .project()
                .load_graph_document(&project, &path, 0)
                .map_err(project_error)?;
            let current = session
                .project()
                .read_graph_editing(&project, &path)
                .map_err(project_error)?;
            if current.state.version.revision.get() != version.revision {
                return Err(conflict());
            }
            let document = session
                .project()
                .read_graph_resource_snapshot(&project, &path)
                .map_err(project_error)?;
            let function = document.function.as_ref().ok_or_else(unavailable)?;
            version = ResourceVersion {
                revision: current.state.version.revision.get(),
                session_id: Some(current.state.version.session_id.to_string()),
            };
            dirty = current.state.dirty;
            ResourceContent::Function {
                signature: FunctionSignatureInspection {
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
                },
            }
        }
        ProjectResourceKind::Database => {
            let basis = session
                .database()
                .capture_query_basis(&yss_database_contract::DatabaseId::from_existing(
                    request.resource.id.clone().into_boxed_str(),
                ))
                .map_err(|_| unavailable())?;
            if basis.declaration_revision().get() != version.revision {
                return Err(conflict());
            }
            dirty = application
                .query_database_edit_state_for_application(
                    project,
                    request.resource.id.clone(),
                    info.revision,
                )
                .map_err(database_error)?
                .is_modified;
            yss_database_runtime::session_api::revalidate_query_basis(session.database(), &basis)
                .map_err(|_| conflict())?;
            ResourceContent::DatabaseMetadata {
                runtime_revision: basis.runtime_revision().get(),
                schema_revision: basis.schema_revision().get(),
            }
        }
        _ => ResourceContent::Metadata,
    };
    if request.resource.kind == ProjectResourceKind::FunctionGraph {
        check_version(session, &request.resource, &version)?;
    } else {
        session
            .project()
            .validate_project_index_version(
                session.project_instance_id(),
                index.publication_revision,
                index.authority_generation,
            )
            .map_err(project_error)?;
    }
    control.check()?;
    Ok(ResourceInspection {
        resource: request.resource,
        name: info.name.clone(),
        version,
        dirty,
        content,
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
    let mut mind_edit = None;
    let mut document_edit = None;
    let mut database_edit = None;
    control.check()?;
    let mutation = match request.edit {
        ResourceEdit::UpdateChart { settings } => chart::update_chart(
            application,
            session,
            &resource,
            &request.version,
            settings,
            operation,
            control,
        )?,
        edit @ (ResourceEdit::CreateTopics { .. }
        | ResourceEdit::UpdateTopics { .. }
        | ResourceEdit::MoveTopics { .. }
        | ResourceEdit::DeleteTopics { .. }
        | ResourceEdit::DuplicateTopics { .. }) => {
            let (mutation, facts) = mind::edit_mind(
                application,
                session,
                &resource,
                &request.version,
                edit,
                control,
            )?;
            mind_edit = Some(facts);
            mutation
        }
        edit @ (ResourceEdit::ReplaceDocumentText { .. }
        | ResourceEdit::AppendDocument { .. }
        | ResourceEdit::WriteDocument { .. }) => {
            let (mutation, facts) = document::edit_document(
                application,
                session,
                &resource,
                &request.version,
                edit,
                control,
            )?;
            document_edit = Some(facts);
            mutation
        }
        edit @ (ResourceEdit::InsertRows { .. }
        | ResourceEdit::UpdateCells { .. }
        | ResourceEdit::DeleteRows { .. }
        | ResourceEdit::CreateColumns { .. }
        | ResourceEdit::RenameColumns { .. }
        | ResourceEdit::DeleteColumns { .. }
        | ResourceEdit::CastColumns { .. }
        | ResourceEdit::SetColumnSemantics { .. }) => {
            let column_names = database::edited_columns(&edit);
            let (edit, item_count) = database::edit_operation(edit)?;
            let result = application
                .mutate_database_for_application(
                    project,
                    resource.id.clone(),
                    ResourceRevision::new(request.version.revision),
                    operation,
                    edit,
                )
                .map_err(database_error)?;
            database_edit = Some(DatabaseEditReceipt {
                item_count,
                inserted_row_ids: result.data.inserted_row_ids,
                column_names,
                dirty: result.data.edit_state.is_modified,
                can_undo: result.data.edit_state.can_undo,
                can_redo: result.data.edit_state.can_redo,
            });
            result.mutation
        }
        ResourceEdit::DatabaseHistory { redo } => {
            application
                .mutate_database_for_application(
                    project,
                    resource.id.clone(),
                    ResourceRevision::new(request.version.revision),
                    operation,
                    if redo {
                        crate::database::DatabaseMutation::Redo
                    } else {
                        crate::database::DatabaseMutation::Undo
                    },
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
                application,
                session,
                resource,
                result.editing.version.revision.get(),
            ));
        }
    };
    let mut receipt = committed(application, session, mutation, publish)?;
    receipt.mind_edit = mind_edit;
    receipt.document_edit = document_edit;
    receipt.database_edit = database_edit;
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
fn graph_receipt(
    application: &ApplicationState,
    session: &ApplicationSession,
    resource: ProjectResourceRef,
    revision: u64,
) -> ResourceMutationReceipt {
    let mut receipt = ResourceMutationReceipt {
        database_edit: None,
        publication_revision: None,
        changes: vec![ResourceChange {
            resource,
            revision,
            revision_kind: ResourceRevisionKind::Resource,
            deleted: false,
        }],
        moves: vec![],
        mind_edit: None,
        document_edit: None,
        resources: Vec::new(),
    };
    attach_committed_metadata(application, session, &mut receipt);
    receipt
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
        application,
        session,
        resource.clone(),
        result.resource_revision.get(),
    ))
}
