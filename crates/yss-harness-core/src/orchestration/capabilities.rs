//! Bind model intentions to the worker grant and actual owner receipts.
use super::*;
use model::{CapabilityInput as Input, ResourceEditInput as Edit};

impl RunExecutor {
    pub(super) async fn bind_input(
        &self,
        input: Input,
    ) -> Result<AutomationCapabilityRequest, CapabilityFailure> {
        {
            let scope = self.scope.lock().unwrap_or_else(|e| e.into_inner());
            crate::agents::authorize_model_capability(&scope, &input)?;
        }
        if input.capability_id().descriptor().effect != ToolEffect::Inspect
            && self
                .evidence
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .blocked_reason
                .as_deref()
                == Some("input_changed")
        {
            return Err(CapabilityFailure::new(
                CapabilityFailureCode::RevisionConflict,
            ));
        }
        use AutomationCapabilityRequest as Request;
        Ok(match input {
            Input::InspectChart(value) => {
                let version = self.read_resource_version(&value.chart.resource());
                Request::InspectChart(InspectChartRequest {
                    chart: value.chart,
                    version,
                })
            }
            Input::UpdateChart(value) => self.bind_resource_edit(
                value.chart.resource(),
                ResourceEdit::UpdateChart {
                    settings: value.settings,
                },
            )?,
            Input::InspectResource(value) => Request::InspectResource(value.into()),
            Input::InspectDocument(value) => {
                self.bind_document_read(model::DocumentReadInput::Outline(value))?
            }
            Input::ReadDocument(value) => {
                self.bind_document_read(model::DocumentReadInput::Text(value))?
            }
            Input::SearchDocument(value) => {
                self.bind_document_read(model::DocumentReadInput::Search(value))?
            }
            Input::ReplaceDocumentText(value) => self.bind_resource_edit(
                value.document.resource(),
                ResourceEdit::ReplaceDocumentText {
                    replacements: value.replacements,
                },
            )?,
            Input::AppendDocument(value) => self.bind_resource_edit(
                value.document.resource(),
                ResourceEdit::AppendDocument { text: value.text },
            )?,
            Input::WriteDocument(value) => self.bind_resource_edit(
                value.document.resource(),
                ResourceEdit::WriteDocument {
                    markdown: value.markdown,
                },
            )?,

            Input::InspectMind(value) => {
                let input = model::MindReadInput::Outline(value);
                let version = self.read_resource_version(&input.mind().resource());
                Request::ReadMind(MindReadRequest { input, version })
            }
            Input::FindTopics(value) => {
                let input = model::MindReadInput::Find(value);
                let version = self.read_resource_version(&input.mind().resource());
                Request::ReadMind(MindReadRequest { input, version })
            }
            Input::InspectTopics(value) => {
                let input = model::MindReadInput::Topics(value);
                let version = self.read_resource_version(&input.mind().resource());
                Request::ReadMind(MindReadRequest { input, version })
            }
            Input::CreateTopics(value) => self.bind_resource_edit(
                value.mind.resource(),
                ResourceEdit::CreateTopics {
                    topics: value.topics,
                    before_id: value.before_id,
                },
            )?,
            Input::UpdateTopics(value) => self.bind_resource_edit(
                value.mind.resource(),
                ResourceEdit::UpdateTopics {
                    topics: value.topics,
                },
            )?,
            Input::MoveTopics(value) => self.bind_resource_edit(
                value.mind.resource(),
                ResourceEdit::MoveTopics {
                    topics: value.topics,
                },
            )?,
            Input::DeleteTopics(value) => self.bind_resource_edit(
                value.mind.resource(),
                ResourceEdit::DeleteTopics {
                    topic_ids: value.topic_ids,
                },
            )?,
            Input::DuplicateTopics(value) => self.bind_resource_edit(
                value.mind.resource(),
                ResourceEdit::DuplicateTopics {
                    topic_ids: value.topic_ids,
                    parent_id: value.parent_id,
                    before_id: value.before_id,
                },
            )?,

            Input::InspectGraph(value) => Request::InspectGraph(value.into()),
            Input::BrowseNodes(value) => Request::BrowseNodes(value),
            Input::InspectNodeType(value) => Request::InspectNodeType(value),
            Input::FindNodes(value) => Request::FindNodes(value),
            Input::FindConstants(value) => Request::FindConstants(value),
            Input::InspectConstants(value) => Request::InspectConstants(value),
            Input::CreateConstants(value) => {
                self.bind_graph_mutation(model::GraphMutationInput::CreateConstants(value))
                    .await?
            }
            Input::UpdateConstants(value) => {
                self.bind_graph_mutation(model::GraphMutationInput::UpdateConstants(value))
                    .await?
            }
            Input::DeleteConstants(value) => {
                self.bind_graph_mutation(model::GraphMutationInput::DeleteConstants(value))
                    .await?
            }

            Input::InspectNodes(value) => Request::InspectNodes(value),
            Input::FindConnections(value) => Request::FindConnections(value),
            Input::SearchKnowledge(value) => Request::SearchKnowledge(value),
            Input::ReadKnowledge(value) => Request::ReadKnowledge(value),
            Input::InspectDatasetSchema(_) | Input::InspectDatasetProfile(_) => {
                return Err(CapabilityFailure::new(
                    CapabilityFailureCode::InvalidRequest,
                ));
            }
            Input::InspectDatabase(value) => {
                self.bind_database_read(model::DatabaseReadInput::Overview(value))
            }
            Input::InspectDatabaseSchema(value) => {
                self.bind_database_read(model::DatabaseReadInput::Schema(value))
            }
            Input::ProfileDatabase(value) => {
                self.bind_database_read(model::DatabaseReadInput::Profile(value))
            }
            Input::ReadDatabaseRows(value) => {
                self.bind_database_read(model::DatabaseReadInput::Rows(value))
            }
            Input::InspectResult(value) => Request::InspectResult(value),
            Input::ReadResultTable(value) => Request::ReadResultTable(value),
            Input::ListResources(value) => Request::ListResources(value),
            Input::InspectUiIntent(value) => Request::InspectUiIntent(value),
            Input::ListGraphResults(value) => Request::ListGraphResults(value),
            Input::RequestUiIntent(value) => Request::RequestUiIntent(RequestUiIntent {
                client_key: self.tools.new_call_key()?,
                input: value,
            }),
            Input::CreateResource(value) => {
                Request::ManageResource(ManageResourceRequest::Create {
                    specification: value.into(),
                })
            }
            Input::ImportDatabase(value) => {
                Request::ManageResource(ManageResourceRequest::Create {
                    specification: ResourceCreation::Database {
                        source: value.source,
                        name: value.name,
                    },
                })
            }
            Input::RenameResource(value) => {
                Request::ManageResource(ManageResourceRequest::Rename {
                    version: self.resource_version(&value.resource)?,
                    resource: value.resource,
                    name: value.name,
                })
            }
            Input::DuplicateResource(value) => {
                Request::ManageResource(ManageResourceRequest::Duplicate {
                    version: self.resource_version(&value.resource)?,
                    resource: value.resource,
                    name: value.name,
                })
            }
            Input::DeleteResource(value) => {
                Request::ManageResource(ManageResourceRequest::Delete {
                    version: self.resource_version(&value.resource)?,
                    resource: value.resource,
                })
            }
            Input::SaveResource(value) => Request::ManageResource(ManageResourceRequest::Save {
                version: self.resource_version(&value.resource)?,
                resource: value.resource,
            }),
            Input::UndoResource(value) => self.bind_resource_history(value.resource, false).await?,
            Input::RedoResource(value) => self.bind_resource_history(value.resource, true).await?,
            Input::InsertRows(value) => self.bind_resource_edit(
                value.database.resource(),
                ResourceEdit::InsertRows {
                    rows: value.rows,
                    before_row_id: value.before_row_id,
                },
            )?,
            Input::UpdateCells(value) => self.bind_resource_edit(
                value.database.resource(),
                ResourceEdit::UpdateCells { cells: value.cells },
            )?,
            Input::DeleteRows(value) => self.bind_resource_edit(
                value.database.resource(),
                ResourceEdit::DeleteRows {
                    row_ids: value.row_ids,
                },
            )?,
            Input::CreateColumns(value) => self.bind_resource_edit(
                value.database.resource(),
                ResourceEdit::CreateColumns {
                    columns: value.columns,
                },
            )?,
            Input::RenameColumns(value) => self.bind_resource_edit(
                value.database.resource(),
                ResourceEdit::RenameColumns {
                    columns: value.columns,
                },
            )?,
            Input::DeleteColumns(value) => self.bind_resource_edit(
                value.database.resource(),
                ResourceEdit::DeleteColumns {
                    columns: value.columns,
                },
            )?,
            Input::CastColumns(value) => self.bind_resource_edit(
                value.database.resource(),
                ResourceEdit::CastColumns {
                    columns: value.columns,
                },
            )?,
            Input::SetColumnSemantics(value) => self.bind_resource_edit(
                value.database.resource(),
                ResourceEdit::SetColumnSemantics {
                    columns: value.columns,
                },
            )?,
            Input::ExportDatabase(value) => Request::ExportDatabase(ExportDatabaseRequest {
                version: self.resource_version(&value.database.resource())?,
                resource: value.database.resource(),
                path: value.path,
                format: value.format,
            }),
            Input::EditResource(value) => {
                let version = self.resource_version(&value.resource)?;
                let edit = match value.edit {
                    Edit::FunctionSignature { signature } => {
                        let revision = self
                            .observations
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .signature_revision(&value.resource.id, &version)
                            .ok_or_else(|| needs_read(&value.resource.id))?;
                        ResourceEdit::FunctionSignature {
                            signature: FunctionSignatureInspection {
                                revision,
                                parameters: signature.parameters,
                                return_type: signature.return_type,
                            },
                        }
                    }
                };
                Request::EditResource(EditResourceRequest {
                    resource: value.resource,
                    version,
                    edit,
                })
            }
            Input::CreateNodes(value) => {
                self.bind_graph_mutation(model::GraphMutationInput::CreateNodes(value))
                    .await?
            }
            Input::UpdateNodes(value) => {
                self.bind_graph_mutation(model::GraphMutationInput::UpdateNodes(value))
                    .await?
            }
            Input::DeleteNodes(value) => {
                self.bind_graph_mutation(model::GraphMutationInput::DeleteNodes(value))
                    .await?
            }
            Input::DuplicateNodes(value) => {
                self.bind_graph_mutation(model::GraphMutationInput::DuplicateNodes(value))
                    .await?
            }
            Input::MoveNodes(value) => {
                self.bind_graph_mutation(model::GraphMutationInput::MoveNodes(value))
                    .await?
            }
            Input::CreateConnections(value) => {
                self.bind_graph_mutation(model::GraphMutationInput::CreateConnections(value))
                    .await?
            }
            Input::UpdateConnections(value) => {
                self.bind_graph_mutation(model::GraphMutationInput::UpdateConnections(value))
                    .await?
            }
            Input::DeleteConnections(value) => {
                self.bind_graph_mutation(model::GraphMutationInput::DeleteConnections(value))
                    .await?
            }
            Input::ApplyGraphEdit(_) | Input::SaveGraph(_) => {
                return Err(CapabilityFailure::new(
                    CapabilityFailureCode::InvalidRequest,
                ));
            }
            Input::ValidateGraph(value) => Request::ValidateGraph(ValidateGraphRequest {
                graph_hash: self.graph_basis(&value.graph.id).await?.hash,
                graph: value.graph,
                node_ids: value.node_ids,
                offset: value.offset,
                limit: value.limit,
            }),
            Input::ExecuteGraph(value) => Request::ExecuteGraph(ExecuteGraphRequest {
                graph_hash: self.graph_basis(&value.graph.id).await?.hash,
                demand: value
                    .demand()
                    .map_err(|error| error.into_failure(CapabilityId::ExecuteGraph))?,
                graph: value.graph,
            }),
        })
    }

    async fn bind_resource_history(
        &self,
        resource: ProjectResourceRef,
        redo: bool,
    ) -> Result<AutomationCapabilityRequest, CapabilityFailure> {
        let version = self.resource_version(&resource)?;
        let edit = match resource.kind {
            ProjectResourceKind::EventGraph | ProjectResourceKind::FunctionGraph => {
                ResourceEdit::GraphHistory {
                    redo,
                    graph_hash: self.graph_basis(&resource.id).await?.hash,
                }
            }
            ProjectResourceKind::Database => ResourceEdit::DatabaseHistory { redo },
            _ => {
                return Err(
                    CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
                        .with_detail("field", "resource.kind"),
                );
            }
        };
        Ok(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource,
                version,
                edit,
            },
        ))
    }

    fn bind_document_read(
        &self,
        input: model::DocumentReadInput,
    ) -> Result<AutomationCapabilityRequest, CapabilityFailure> {
        let resource = input.document().resource();
        let granted = self.read_resource_version(&resource);
        let version = if matches!(&input, model::DocumentReadInput::Outline(value) if value.section.is_none() && value.offset == 0)
        {
            granted
        } else {
            let observations = self
                .observations
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let current = match granted {
                Some(version) => Some(version),
                None => observations.version(&resource, false)?,
            };
            if let Some(section) = input.section() {
                let basis = observations
                    .document_section_version(&resource.id, section)
                    .ok_or_else(|| needs_read(&resource.id))?;
                if current.as_ref().is_some_and(|current| current != &basis) {
                    return Err(
                        CapabilityFailure::new(CapabilityFailureCode::RevisionConflict)
                            .with_detail("reason", "document_section_changed")
                            .with_detail("resourceId", &resource.id),
                    );
                }
                Some(basis)
            } else {
                current
            }
        };
        Ok(AutomationCapabilityRequest::ReadDocument(
            DocumentReadRequest { input, version },
        ))
    }

    fn read_resource_version(&self, resource: &ProjectResourceRef) -> Option<ResourceVersion> {
        self.scope
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .task
            .as_ref()
            .and_then(|task| {
                task.resources
                    .iter()
                    .find(|grant| grant.resource == *resource)
            })
            .and_then(|grant| grant.version.clone())
    }

    fn bind_database_read(&self, input: model::DatabaseReadInput) -> AutomationCapabilityRequest {
        let version = self.read_resource_version(&input.database().resource());
        AutomationCapabilityRequest::ReadDatabase(DatabaseReadRequest { input, version })
    }

    fn bind_resource_edit(
        &self,
        resource: ProjectResourceRef,
        edit: ResourceEdit,
    ) -> Result<AutomationCapabilityRequest, CapabilityFailure> {
        Ok(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                version: self.resource_version(&resource)?,
                resource,
                edit,
            },
        ))
    }

    async fn bind_graph_mutation(
        &self,
        input: model::GraphMutationInput,
    ) -> Result<AutomationCapabilityRequest, CapabilityFailure> {
        let basis = self.graph_basis(&input.graph().id).await?;
        let digest = yss_canonical_hash::hash_canonical(
            "yssbi.harness.graph-intention",
            &(&basis.version, &basis.hash, &basis.inputs, &input),
        )
        .map_err(|_| persistence_failure())?;
        Ok(AutomationCapabilityRequest::GraphMutation(
            GraphMutationRequest {
                input,
                base_revision: basis.version.revision,
                graph_hash: basis.hash,
                client_key: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
            },
        ))
    }

    pub(super) fn graph_observation(
        &self,
        request: &AutomationCapabilityRequest,
    ) -> Option<String> {
        use AutomationCapabilityRequest as Request;
        let path = match request {
            Request::InspectGraph(value) => &value.graph.id,
            Request::FindNodes(value) => &value.graph.id,
            Request::FindConstants(value) => &value.graph.id,
            Request::InspectConstants(value) => &value.graph.id,

            Request::InspectNodes(value) => &value.graph.id,
            Request::FindConnections(value) => &value.graph.id,
            Request::ApplyGraphEdit(value) => &value.graph_path,
            Request::GraphMutation(value) => &value.input.graph().id,
            Request::ValidateGraph(value) => &value.graph.id,
            Request::ExecuteGraph(value) => &value.graph.id,
            Request::SaveGraph(value) => &value.graph_path,
            Request::InspectResource(value) => &value.resource.id,
            Request::EditResource(value) => &value.resource.id,
            _ => return None,
        };
        self.observations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .graph_inputs(path)
    }

    fn resource_version(
        &self,
        resource: &ProjectResourceRef,
    ) -> Result<ResourceVersion, CapabilityFailure> {
        self.scope
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .task
            .as_ref()
            .and_then(|task| {
                task.resources
                    .iter()
                    .find(|grant| &grant.resource == resource)
            })
            .and_then(|grant| grant.version.clone())
            .ok_or_else(|| needs_read(&resource.id))
    }

    async fn graph_basis(&self, path: &str) -> Result<observations::GraphBasis, CapabilityFailure> {
        let root_manager = {
            let scope = self.scope.lock().unwrap_or_else(|error| error.into_inner());
            scope.role == AgentRole::Manager && scope.task.is_none()
        };
        if root_manager {
            // Manager only registers read-only graph validation. Capture its actual
            // read rather than requiring a worker grant or borrowing a worker's baseline.
            return self
                .read_graph_basis(GraphResourceRef::for_path(path))
                .await;
        }
        let resource = self
            .scope
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .task
            .as_ref()
            .and_then(|task| {
                task.resources.iter().find(|grant| {
                    grant.resource.id == path
                        && matches!(
                            grant.resource.kind,
                            ProjectResourceKind::EventGraph | ProjectResourceKind::FunctionGraph
                        )
                })
            })
            .map(|grant| grant.resource.clone())
            .ok_or_else(|| needs_read(path))?;
        let version = self.resource_version(&resource)?;
        if let Some(basis) = self
            .observations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .graph_basis(path, &version)
        {
            return Ok(basis);
        }
        let observed = self
            .read_graph_basis(GraphResourceRef::for_path(path))
            .await?;
        if crate::agents::resource_version_matches(resource.kind, &version, &observed.version) {
            if let Some(access) = self
                .scope
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .task
                .as_mut()
                .and_then(|task| {
                    task.resources
                        .iter_mut()
                        .find(|access| access.resource == resource)
                })
            {
                access.version = Some(observed.version.clone());
            }
            return Ok(observed);
        }
        Err(
            CapabilityFailure::new(CapabilityFailureCode::RevisionConflict)
                .with_detail("resourceId", path),
        )
    }

    async fn read_graph_basis(
        &self,
        graph: GraphResourceRef,
    ) -> Result<observations::GraphBasis, CapabilityFailure> {
        let outcome = self
            .execute_tool(AutomationCapabilityRequest::InspectGraph(
                InspectGraphRequest::summary(graph.clone()),
            ))
            .await?;
        let mut observations = self
            .observations
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        observations.record(&outcome.result);
        let version = observations
            .version(&graph.resource(), false)?
            .ok_or_else(|| needs_read(&graph.id))?;
        observations
            .graph_basis(&graph.id, &version)
            .ok_or_else(|| needs_read(&graph.id))
    }
}

fn needs_read(resource: &str) -> CapabilityFailure {
    rejected("resource_read_required").with_detail("resourceId", resource)
}
