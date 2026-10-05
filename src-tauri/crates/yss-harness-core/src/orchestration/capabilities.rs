//! Bind model intentions to the worker grant and actual owner receipts.
use super::*;
use model::{CapabilityInput as Input, ManageResourceInput as Manage, ResourceEditInput as Edit};

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
            Input::InspectResource(value) => Request::InspectResource(value),
            Input::InspectGraph(value) => Request::InspectGraph(value.into()),
            Input::SearchNodeCatalog(value) => Request::SearchNodeCatalog(value),
            Input::SearchKnowledge(value) => Request::SearchKnowledge(value),
            Input::ReadKnowledge(value) => Request::ReadKnowledge(value),
            Input::InspectDatasetSchema(value) => Request::InspectDatasetSchema(value),
            Input::InspectDatasetProfile(value) => Request::InspectDatasetProfile(value),
            Input::InspectResult(value) => Request::InspectResult(value),
            Input::InspectProject(value) => Request::InspectProject(value),
            Input::InspectUiIntent(value) => Request::InspectUiIntent(value),
            Input::ListGraphResults(value) => Request::ListGraphResults(value),
            Input::RequestUiIntent(value) => Request::RequestUiIntent(RequestUiIntent {
                client_key: self.tools.new_call_key()?,
                intent: value.intent,
            }),
            Input::ManageResource(value) => Request::ManageResource(match value {
                Manage::Create { specification } => ManageResourceRequest::Create { specification },
                Manage::Rename { resource, name } => ManageResourceRequest::Rename {
                    version: self.resource_version(&resource)?,
                    resource,
                    name,
                },
                Manage::Duplicate { resource } => ManageResourceRequest::Duplicate {
                    version: self.resource_version(&resource)?,
                    resource,
                },
                Manage::Delete { resource } => ManageResourceRequest::Delete {
                    version: self.resource_version(&resource)?,
                    resource,
                },
                Manage::Save { resource } => ManageResourceRequest::Save {
                    version: self.resource_version(&resource)?,
                    resource,
                },
            }),
            Input::ExportDataset(value) => Request::ExportDataset(ExportDatasetRequest {
                version: self.resource_version(&value.resource)?,
                resource: value.resource,
                path: value.path,
                format: value.format,
            }),
            Input::EditResource(value) => {
                let version = self.resource_version(&value.resource)?;
                let edit = match value.edit {
                    Edit::Chart { settings } => ResourceEdit::Chart { settings },
                    Edit::Mind { operations } => ResourceEdit::Mind { operations },
                    Edit::Doc { operations } => ResourceEdit::Doc { operations },
                    Edit::Database { operation } => ResourceEdit::Database { operation },
                    Edit::GraphHistory { redo } => ResourceEdit::GraphHistory {
                        redo,
                        graph_hash: self.graph_basis(&value.resource.id).await?.hash,
                    },
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
            Input::ApplyGraphEdit(value) => {
                let basis = self.graph_basis(&value.graph_path).await?;
                // Retries at the same baseline recover the same owner operation. A later
                // committed baseline is a different operation, even with identical arguments.
                let digest = yss_canonical_hash::hash_canonical(
                    "yssbi.harness.graph-intention",
                    &(&basis.version, &basis.hash, &basis.inputs, &value),
                )
                .map_err(|_| persistence_failure())?;
                Request::ApplyGraphEdit(ApplyGraphEditRequest {
                    graph_path: value.graph_path,
                    base_revision: basis.version.revision,
                    graph_hash: basis.hash,
                    client_key: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
                    locale: value.locale,
                    operations: value.operations,
                })
            }
            Input::ValidateGraph(value) => Request::ValidateGraph(ValidateGraphRequest {
                graph_hash: self.graph_basis(&value.graph_path).await?.hash,
                graph_path: value.graph_path,
            }),
            Input::ExecuteGraph(value) => Request::ExecuteGraph(ExecuteGraphRequest {
                graph_hash: self.graph_basis(&value.graph_path).await?.hash,
                graph_path: value.graph_path,
                demand: value.demand,
            }),
            Input::SaveGraph(value) => Request::SaveGraph(SaveGraphRequest {
                graph_hash: self.graph_basis(&value.graph_path).await?.hash,
                graph_path: value.graph_path,
            }),
        })
    }

    pub(super) fn graph_observation(
        &self,
        request: &AutomationCapabilityRequest,
    ) -> Option<String> {
        use AutomationCapabilityRequest as Request;
        let path = match request {
            Request::InspectGraph(value) => &value.graph_path,
            Request::ApplyGraphEdit(value) => &value.graph_path,
            Request::ValidateGraph(value) => &value.graph_path,
            Request::ExecuteGraph(value) => &value.graph_path,
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
        let outcome = self
            .execute_tool(AutomationCapabilityRequest::InspectGraph(
                InspectGraphRequest {
                    limit: 1,
                    ..InspectGraphRequest::overview(path)
                },
            ))
            .await?;
        let mut observations = self.observations.lock().unwrap_or_else(|e| e.into_inner());
        observations.record(&outcome.result);
        observations
            .graph_basis(path, &version)
            .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::RevisionConflict))
    }
}

fn needs_read(resource: &str) -> CapabilityFailure {
    rejected("resource_read_required").with_detail("resourceId", resource)
        .with_detail("nextStep", "Inspect the resource contents before editing or saving. The host will capture the required baseline.")
}
