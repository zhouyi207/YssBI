//! Resource identity, lifecycle dispatch and receipts shared by every Harness resource tool.
use super::{map_session_capture_error, map_session_revalidation_error};
use crate::events::CommittedResourceMutation;
use crate::session::{ApplicationSession, ApplicationState};
use yss_chart_document::ChartResourcePath;
use yss_graph_document::{GraphResourceKind, GraphResourcePath};
use yss_harness_contract::*;
use yss_project::file_resources::FileCommand;
use yss_project_history::{ResourceDocumentPatch, ResourceKey, ResourceLifecycleKind};
use yss_project_identity::{OperationId, ResourceRevision};
use yss_project_model::{
    doc::{DocDocument, DocPath},
    file::FileVersion,
    mind::{MindDocument, MindPath},
};

mod chart;
pub(super) use chart::inspect_chart;
mod content;
mod database;
mod document;
pub(super) use document::read_document;
mod mind;
pub(super) use mind::read_mind;
#[cfg(test)]
mod tests;
pub(super) use content::{edit_resource, inspect_resource};
pub(super) use database::export_database;
pub(super) use database::read_database;
pub(super) use database::semantic_to_contract;

type Publication<'a> = &'a mut dyn FnMut(&CommittedResourceMutation);
type Result<T> = std::result::Result<T, CapabilityFailure>;

impl ApplicationState {
    /// Resolve explicit message references against the existing project index.
    /// The Application resolver dispatches this filesystem read on its blocking pool.
    pub fn resolve_harness_resources(
        &self,
        binding: &ProjectSessionBinding,
        references: &[ProjectResourceRef],
    ) -> Result<Vec<HarnessResourceReference>> {
        if references.is_empty() {
            return Ok(Vec::new());
        }
        let session = self.capture_session().map_err(map_session_capture_error)?;
        if session.project_instance_id() != binding.project_instance_id()
            || session.project_session_id() != binding.project_session_id()
        {
            return Err(CapabilityFailure::new(
                CapabilityFailureCode::ProjectSessionChanged,
            ));
        }
        let index = read_index(&session)?;
        let resources = resource_entries(&index);
        let mut seen = std::collections::BTreeSet::new();
        let selected = references
            .iter()
            .map(|reference| {
                if !seen.insert(reference) {
                    return Err(invalid("resources"));
                }
                let entry = resources
                    .iter()
                    .find(|entry| &entry.resource == reference)
                    .ok_or_else(unavailable)?;
                Ok(HarnessResourceReference {
                    resource: reference.clone(),
                    name: entry.display_name.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.revalidate_captured_session(&session)
            .map_err(map_session_revalidation_error)?;
        session
            .project()
            .validate_project_index_version(
                session.project_instance_id(),
                index.publication_revision,
                index.authority_generation,
            )
            .map_err(project_error)?;
        Ok(selected)
    }
}

fn invalid(field: &str) -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::InvalidRequest).with_detail("field", field)
}
fn conflict() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::RevisionConflict)
}
fn unavailable() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::ResourceUnavailable)
}
fn reference(kind: ProjectResourceKind, id: impl Into<String>) -> ProjectResourceRef {
    ProjectResourceRef {
        kind,
        id: id.into(),
    }
}

fn resource_entries(
    index: &yss_project::resource_catalog::ResourceCatalog,
) -> Vec<ProjectResourceInspection> {
    index
        .resources
        .iter()
        .map(|entry| ProjectResourceInspection {
            resource: entry.resource.clone(),
            display_name: entry.name.clone(),
            revision: entry.revision.get(),
        })
        .collect()
}

pub(super) fn read_index(
    session: &ApplicationSession,
) -> Result<yss_project::resource_catalog::ResourceCatalog> {
    session
        .project()
        .read_resource_catalog(session.project_instance_id())
        .map_err(super::map_project_inspection_error)
}
pub(super) fn project_inspection(
    session: &ApplicationSession,
    request: ListResourcesRequest,
) -> Result<ProjectInspection> {
    let index = read_index(session)?;
    let query = request.query.as_deref().unwrap_or_default().to_lowercase();
    let matched = resource_entries(&index)
        .into_iter()
        .filter(|entry| {
            (request.kinds.is_empty() || request.kinds.contains(&entry.resource.kind))
                && (query.is_empty()
                    || entry.display_name.to_lowercase().contains(&query)
                    || entry.resource.id.to_lowercase().contains(&query))
        })
        .collect::<Vec<_>>();
    let total = matched.len();
    let resources = matched
        .into_iter()
        .skip(request.offset)
        .take(request.limit)
        .collect::<Vec<_>>();
    let page = InspectionPage::known(request.offset, resources.len(), total);
    session
        .project()
        .validate_project_index_version(
            session.project_instance_id(),
            index.publication_revision,
            index.authority_generation,
        )
        .map_err(project_error)?;
    Ok(ProjectInspection {
        project_name: index.project_name,
        publication_revision: index.publication_revision,
        resources,
        page,
    })
}
fn metadata(
    index: &yss_project::resource_catalog::ResourceCatalog,
    resource: &ProjectResourceRef,
) -> Result<ProjectResourceInspection> {
    resource_entries(index)
        .into_iter()
        .find(|entry| entry.resource == *resource)
        .ok_or_else(unavailable)
}
fn check_version(
    session: &ApplicationSession,
    resource: &ProjectResourceRef,
    version: &ResourceVersion,
) -> Result<()> {
    let catalog = read_index(session)?;
    let current = catalog
        .resources
        .iter()
        .find(|entry| entry.resource == *resource)
        .ok_or_else(unavailable)?;
    if current.revision.get() != version.revision
        || (version.session_id.is_some() && current.session_id != version.session_id)
    {
        return Err(conflict());
    }
    Ok(())
}

fn graph_path(resource: &ProjectResourceRef) -> Result<GraphResourcePath> {
    let path = GraphResourcePath::new(&resource.id).map_err(|_| invalid("resource.id"))?;
    let kind = match path.kind() {
        GraphResourceKind::EventGraph => ProjectResourceKind::EventGraph,
        GraphResourceKind::FunctionGraph => ProjectResourceKind::FunctionGraph,
    };
    if resource.kind != kind {
        return Err(invalid("resource.kind"));
    }
    Ok(path)
}
fn chart_path(resource: &ProjectResourceRef) -> Result<ChartResourcePath> {
    ChartResourcePath::parse(&resource.id).map_err(|_| invalid("resource.id"))
}
fn mind_path(resource: &ProjectResourceRef) -> Result<MindPath> {
    MindPath::parse(&resource.id).map_err(|_| invalid("resource.id"))
}
fn doc_path(resource: &ProjectResourceRef) -> Result<DocPath> {
    DocPath::parse(&resource.id).map_err(|_| invalid("resource.id"))
}
fn file_version(version: &ResourceVersion) -> Result<FileVersion> {
    Ok(FileVersion {
        session_id: version
            .session_id
            .clone()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| invalid("version.sessionId"))?,
        revision: ResourceRevision::new(version.revision),
    })
}
fn file_resource_kind(kind: ResourceLifecycleKind) -> ProjectResourceKind {
    match kind {
        ResourceLifecycleKind::EventGraph => ProjectResourceKind::EventGraph,
        ResourceLifecycleKind::FunctionGraph => ProjectResourceKind::FunctionGraph,
        ResourceLifecycleKind::Chart => ProjectResourceKind::Chart,
        ResourceLifecycleKind::Mind => ProjectResourceKind::Mind,
        ResourceLifecycleKind::Doc => ProjectResourceKind::Doc,
    }
}
fn mutation_receipt(mutation: &CommittedResourceMutation) -> Result<ResourceMutationReceipt> {
    let changes = mutation
        .deltas
        .iter()
        .map(|delta| {
            let (resource, revision_kind) = match &delta.resource {
                ResourceKey::Graph(path) => (
                    reference(
                        match path.kind() {
                            GraphResourceKind::EventGraph => ProjectResourceKind::EventGraph,
                            GraphResourceKind::FunctionGraph => ProjectResourceKind::FunctionGraph,
                        },
                        path.as_str(),
                    ),
                    ResourceRevisionKind::Resource,
                ),
                ResourceKey::Function(key) => (
                    reference(ProjectResourceKind::FunctionGraph, key.0.to_string()),
                    ResourceRevisionKind::FunctionSignature,
                ),
                ResourceKey::Chart(key) => (
                    reference(ProjectResourceKind::Chart, key.0.to_string()),
                    ResourceRevisionKind::Resource,
                ),
                ResourceKey::Mind(key) => (
                    reference(ProjectResourceKind::Mind, key.0.to_string()),
                    ResourceRevisionKind::Resource,
                ),
                ResourceKey::Doc(key) => (
                    reference(ProjectResourceKind::Doc, key.0.to_string()),
                    ResourceRevisionKind::Resource,
                ),
                ResourceKey::Database(_) => {
                    // A publication key is not a DatabaseId; the committed declaration owns the ID.
                    let ResourceDocumentPatch::Database(patch) = &delta.payload else {
                        return Err(CapabilityFailure::new(
                            CapabilityFailureCode::OutcomeUnknown,
                        ));
                    };
                    let declaration =
                        patch
                            .after
                            .as_ref()
                            .or(patch.before.as_ref())
                            .ok_or_else(|| {
                                CapabilityFailure::new(CapabilityFailureCode::OutcomeUnknown)
                            })?;
                    (
                        reference(ProjectResourceKind::Database, declaration.id.as_str()),
                        ResourceRevisionKind::Resource,
                    )
                }
            };
            let deleted = match &delta.payload {
                ResourceDocumentPatch::ResourceLifecycle(patch) => patch.after.is_none(),
                ResourceDocumentPatch::Database(patch) => patch.after.is_none(),
                _ => false,
            };
            Ok(ResourceChange {
                resource,
                revision: delta.to_revision.get(),
                revision_kind,
                deleted,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ResourceMutationReceipt {
        database_edit: None,
        publication_revision: Some(mutation.publication_revision),
        changes,
        moves: mutation
            .moves
            .iter()
            .map(|item| ResourceMove {
                from: reference(file_resource_kind(item.kind), item.from.to_string()),
                to: reference(file_resource_kind(item.kind), item.to.to_string()),
            })
            .collect(),
        mind_edit: None,
        document_edit: None,
        resources: Vec::new(),
    })
}
fn committed(
    application: &ApplicationState,
    session: &ApplicationSession,
    mutation: CommittedResourceMutation,
    publish: Publication<'_>,
) -> Result<ResourceMutationReceipt> {
    let mut receipt = mutation_receipt(&mutation)?;
    attach_committed_metadata(application, session, &mut receipt);
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| publish(&mutation))).is_err() {
        tracing::warn!(
            domain = "Application",
            event = "harness_resource_publication_panicked",
            "Resource committed but its publication callback panicked"
        );
    }
    Ok(receipt)
}

fn attach_committed_metadata(
    application: &ApplicationState,
    session: &ApplicationSession,
    receipt: &mut ResourceMutationReceipt,
) {
    // Metadata is a continuation aid, not the commit verdict. Never turn a real
    // commit into a failure or advance its baseline to a later concurrent edit.
    let Ok(catalog) = read_index(session) else {
        return;
    };
    let states = receipt
        .changes
        .iter()
        .filter(|change| !change.deleted && change.revision_kind == ResourceRevisionKind::Resource)
        .filter_map(|change| {
            let metadata = catalog.resources.iter().find(|entry| {
                entry.resource == change.resource && entry.revision.get() == change.revision
            })?;
            let dirty = metadata.dirty.or_else(|| {
                application
                    .query_database_edit_state_for_application(
                        session.project_instance_id().clone(),
                        metadata.resource.id.clone(),
                        metadata.revision,
                    )
                    .ok()
                    .map(|state| state.is_modified)
            });
            Some(ResourceMutationState {
                resource: metadata.resource.clone(),
                name: metadata.name.clone(),
                version: ResourceVersion {
                    revision: metadata.revision.get(),
                    session_id: metadata.session_id.clone(),
                },
                dirty,
                root_topic_id: metadata.root_topic_id.clone(),
            })
        })
        .collect();
    if session
        .project()
        .validate_project_index_version(
            session.project_instance_id(),
            catalog.publication_revision,
            catalog.authority_generation,
        )
        .is_ok()
    {
        receipt.resources = states;
    }
}

pub(super) fn manage_resource(
    application: &ApplicationState,
    session: &ApplicationSession,
    request: ManageResourceRequest,
    control: &CapabilityControl,
    publish: Publication<'_>,
) -> Result<ResourceMutationReceipt> {
    let project = session.project_instance_id().clone();
    let operation = OperationId::new();
    if let ManageResourceRequest::Create { specification } = request {
        control.check()?;
        let mutation = match specification {
            ResourceCreation::EventGraph { name } => application
                .create_event_graph(project, name, operation)
                .map_err(graph_error)?,
            ResourceCreation::FunctionGraph { name } => application
                .create_function_graph(project, name, operation)
                .map_err(graph_error)?,
            ResourceCreation::Chart { name } => application
                .create_chart_resource(project, operation, name, None)
                .map_err(chart_error)?,
            ResourceCreation::Mind { name } => {
                application
                    .apply_mind_command(
                        project,
                        operation,
                        FileCommand::<MindDocument>::Create { name },
                    )
                    .map_err(file_error)?
                    .mutation
            }
            ResourceCreation::Doc { name } => {
                application
                    .apply_doc_command(
                        project,
                        operation,
                        FileCommand::<DocDocument>::Create { name },
                    )
                    .map_err(file_error)?
                    .mutation
            }
            ResourceCreation::Database { source, name } => {
                application
                    .load_database_for_application(
                        project,
                        operation,
                        database::import_source(source),
                        name,
                    )
                    .map_err(database_error)?
                    .mutation
            }
        };
        return committed(application, session, mutation, publish);
    }
    let (resource, version) = match &request {
        ManageResourceRequest::Rename {
            resource, version, ..
        }
        | ManageResourceRequest::Duplicate {
            resource, version, ..
        }
        | ManageResourceRequest::Delete { resource, version }
        | ManageResourceRequest::Save { resource, version } => (resource, version),
        ManageResourceRequest::Create { .. } => unreachable!(),
    };
    check_version(session, resource, version)?;
    control.check()?;
    let expected = ResourceRevision::new(version.revision);
    let mutation = match resource.kind {
        ProjectResourceKind::EventGraph | ProjectResourceKind::FunctionGraph => {
            let path = graph_path(resource)?;
            match request {
                ManageResourceRequest::Rename { name, .. } => application
                    .rename_graph_resource(project, path, expected, name, 0, operation)
                    .map_err(graph_error)?,
                ManageResourceRequest::Duplicate { name, .. } => application
                    .duplicate_graph_resource(project, path, expected, operation, name)
                    .map_err(graph_error)?,
                ManageResourceRequest::Delete { .. } => application
                    .remove_graph_resource(project, path, expected, operation)
                    .map_err(graph_error)?,
                ManageResourceRequest::Save { .. } => {
                    return content::save_graph(application, session, resource, version);
                }
                _ => unreachable!(),
            }
        }
        ProjectResourceKind::Chart => {
            let path = chart_path(resource)?;
            match request {
                ManageResourceRequest::Rename { name, .. } => application
                    .rename_chart_resource(project, operation, path, expected, name, 0)
                    .map_err(chart_error)?,
                ManageResourceRequest::Duplicate { name, .. } => application
                    .duplicate_chart_resource(project, operation, path, expected, name)
                    .map_err(chart_error)?,
                ManageResourceRequest::Delete { .. } => application
                    .remove_chart_resource(project, operation, path, expected)
                    .map_err(chart_error)?,
                ManageResourceRequest::Save { .. } => {
                    // Charts have no Rust editing buffer; saving the current persisted version is a no-op.
                    let mut receipt = ResourceMutationReceipt {
                        database_edit: None,
                        publication_revision: None,
                        changes: vec![ResourceChange {
                            resource: resource.clone(),
                            revision: version.revision,
                            revision_kind: ResourceRevisionKind::Resource,
                            deleted: false,
                        }],
                        moves: vec![],
                        mind_edit: None,
                        document_edit: None,
                        resources: Vec::new(),
                    };
                    attach_committed_metadata(application, session, &mut receipt);
                    return Ok(receipt);
                }
                _ => unreachable!(),
            }
        }
        ProjectResourceKind::Mind => {
            let path = mind_path(resource)?;
            let version = file_version(version)?;
            let command = match request {
                ManageResourceRequest::Rename { name, .. } => FileCommand::Rename {
                    path,
                    version,
                    name,
                },
                ManageResourceRequest::Duplicate { name, .. } => FileCommand::Duplicate {
                    path,
                    version,
                    name,
                },
                ManageResourceRequest::Delete { .. } => FileCommand::Delete { path, version },
                ManageResourceRequest::Save { .. } => FileCommand::Save { path, version },
                _ => unreachable!(),
            };
            application
                .apply_mind_command(project, operation, command)
                .map_err(file_error)?
                .mutation
        }
        ProjectResourceKind::Doc => {
            let path = doc_path(resource)?;
            let version = file_version(version)?;
            let command = match request {
                ManageResourceRequest::Rename { name, .. } => FileCommand::Rename {
                    path,
                    version,
                    name,
                },
                ManageResourceRequest::Duplicate { name, .. } => FileCommand::Duplicate {
                    path,
                    version,
                    name,
                },
                ManageResourceRequest::Delete { .. } => FileCommand::Delete { path, version },
                ManageResourceRequest::Save { .. } => FileCommand::Save { path, version },
                _ => unreachable!(),
            };
            application
                .apply_doc_command(project, operation, command)
                .map_err(file_error)?
                .mutation
        }
        ProjectResourceKind::Database => {
            let id = resource.id.clone();
            match request {
                ManageResourceRequest::Rename { name, .. } => {
                    application
                        .rename_database_for_application(project, id, expected, name, operation)
                        .map_err(database_error)?
                        .mutation
                }
                ManageResourceRequest::Delete { .. } => {
                    application
                        .delete_database_for_application(project, id, expected, operation)
                        .map_err(database_error)?
                        .mutation
                }
                ManageResourceRequest::Duplicate { name, .. } => {
                    application
                        .duplicate_database_for_application(project, id, expected, operation, name)
                        .map_err(database_error)?
                        .mutation
                }
                ManageResourceRequest::Save { .. } => {
                    application
                        .save_database_for_application(project, id, expected, operation)
                        .map_err(database_error)?
                        .mutation
                }
                _ => unreachable!(),
            }
        }
    };
    committed(application, session, mutation, publish)
}

fn project_error(error: yss_project::ProjectOperationError) -> CapabilityFailure {
    use yss_project::ProjectOperationError as E;
    let code = match &error {
        E::StaleProjectLifecycle { .. } => CapabilityFailureCode::ProjectSessionChanged,
        E::StaleResourceLifecycle { .. }
        | E::CatalogResourceStale { .. }
        | E::ResourceRevisionConflict { .. } => CapabilityFailureCode::RevisionConflict,
        E::ProjectLifecycleAdmissionClosed { .. } | E::ProjectRecoveryRequired { .. } => {
            CapabilityFailureCode::ProjectSessionUnavailable
        }
        E::ChartNotFound { .. } => CapabilityFailureCode::ResourceUnavailable,
        E::TransactionCommitFailed { .. } | E::TransactionRollbackFailed { .. } => {
            CapabilityFailureCode::OutcomeUnknown
        }
        _ => CapabilityFailureCode::MutationRejected,
    };
    CapabilityFailure::new(code).with_detail("reason", error.code())
}
fn file_error(error: crate::file_resources::FileApplicationError) -> CapabilityFailure {
    match error {
        crate::file_resources::FileApplicationError::Project(error) => project_error(error),
        crate::file_resources::FileApplicationError::SessionCapture(error) => {
            map_session_capture_error(error)
        }
        crate::file_resources::FileApplicationError::SessionChanged(error) => {
            map_session_revalidation_error(error)
        }
    }
}
fn chart_error(error: crate::chart::ChartApplicationError) -> CapabilityFailure {
    match error {
        crate::chart::ChartApplicationError::Project(error) => project_error(error),
        crate::chart::ChartApplicationError::SessionCapture(error) => {
            map_session_capture_error(error)
        }
        crate::chart::ChartApplicationError::SessionChanged(error) => {
            map_session_revalidation_error(error)
        }
    }
}
fn graph_error(
    error: crate::graph::resources::ResourceMutationApplicationError,
) -> CapabilityFailure {
    use crate::graph::resources::ResourceMutationApplicationError as E;
    match error {
        E::Project(error) => project_error(error),
        E::SessionCapture(error) => map_session_capture_error(error),
        E::SessionChanged(error) => map_session_revalidation_error(error),
        E::Resource(yss_project_history::ProjectResourceMutationError::StaleRevision {
            ..
        }) => conflict(),
        E::EditingBusy => CapabilityFailure::new(CapabilityFailureCode::InvocationConflict),
        _ => CapabilityFailure::new(CapabilityFailureCode::MutationRejected),
    }
}
fn database_error(error: crate::database::DatabaseUseCaseError) -> CapabilityFailure {
    use crate::database::{DatabaseOperationError as D, DatabaseUseCaseError as E};
    match error {
        E::SessionCapture(error) => map_session_capture_error(error),
        E::SessionChanged(error) => map_session_revalidation_error(error),
        E::Database(D::StaleRevision { .. }) => conflict(),
        E::Database(D::NotFound { .. }) => unavailable(),
        E::Database(D::Project { source, .. }) => project_error(source),
        E::SessionRefresh(_) => CapabilityFailure::new(CapabilityFailureCode::OutcomeUnknown),
        E::Database(D::Internal(error))
            if error.operation()
                == crate::database::DatabaseApplicationOperation::ExportPublicationUncertain =>
        {
            CapabilityFailure::new(CapabilityFailureCode::OutcomeUnknown)
        }
        _ => CapabilityFailure::new(CapabilityFailureCode::MutationRejected),
    }
}
