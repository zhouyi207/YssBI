use std::collections::BTreeMap;
use std::sync::Arc;

use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
    DatabaseDeclarationObservationSet, DatabaseDeclarationRevision, DatabaseId,
};
use yss_database_runtime::session_api::{
    catalog_snapshot, revalidate_catalog_snapshot, revalidate_declaration_observations,
};
use yss_function_editor_projection::parse_function_data_type;
use yss_graph_catalog::{
    CatalogResourceEntry, CatalogResourcePath, LocalizedCatalog, ResourceBoundCreateArgs,
};
use yss_graph_document::{GraphDocument, GraphResourcePath, GraphRevision, PortAddress};
use yss_graph_registry::RegistryFingerprint;
use yss_graph_resource_contract::{FunctionSignature, GraphResourceId, VariableValueContract};
use yss_graph_runtime::GraphRuntimeCatalogError;
use yss_project::ProjectIndex;
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::ProjectInstanceId;

use super::execution::session_slot::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use super::graph_contracts::{
    GraphContractMappingError, ProjectGraphResourceSnapshot, build_resource_catalog,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizedCatalogRequest {
    project_instance_id: ProjectInstanceId,
    locale: Box<str>,
}

impl LocalizedCatalogRequest {
    pub fn new(project_instance_id: ProjectInstanceId, locale: impl Into<Box<str>>) -> Self {
        Self {
            project_instance_id,
            locale: locale.into(),
        }
    }

    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibleCatalogRequest {
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    graph_revision: GraphRevision,
    source_port: PortAddress,
    locale: Box<str>,
}

impl CompatibleCatalogRequest {
    pub fn new(
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        graph_revision: GraphRevision,
        source_port: PortAddress,
        locale: impl Into<Box<str>>,
    ) -> Self {
        Self {
            project_instance_id,
            graph_path,
            graph_revision,
            source_port,
            locale: locale.into(),
        }
    }

    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub fn graph_path(&self) -> &GraphResourcePath {
        &self.graph_path
    }

    pub const fn graph_revision(&self) -> GraphRevision {
        self.graph_revision
    }

    pub fn source_port(&self) -> &PortAddress {
        &self.source_port
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }
}

#[derive(Debug, thiserror::Error)]
#[error("project catalog read failed")]
pub struct ProjectCatalogReadSource {
    #[source]
    reason: ProjectCatalogReadSourceKind,
}

#[derive(Debug, thiserror::Error)]
enum ProjectCatalogReadSourceKind {
    #[error("project filesystem read failed")]
    Filesystem(#[source] ProjectFilesystemError),
    #[error("project graph declaration path is invalid")]
    InvalidGraphPath(#[source] yss_graph_document::GraphResourcePathError),
    #[error("project function declaration type is invalid")]
    InvalidFunctionType,
    #[error("project catalog declaration facts are invalid")]
    InvalidDeclarationFacts,
}

impl ProjectCatalogReadSource {
    fn filesystem(error: ProjectFilesystemError) -> Self {
        Self {
            reason: ProjectCatalogReadSourceKind::Filesystem(error),
        }
    }

    fn invalid_graph_path(error: yss_graph_document::GraphResourcePathError) -> Self {
        Self {
            reason: ProjectCatalogReadSourceKind::InvalidGraphPath(error),
        }
    }

    fn invalid_function_type() -> Self {
        Self {
            reason: ProjectCatalogReadSourceKind::InvalidFunctionType,
        }
    }

    fn invalid_declaration_facts() -> Self {
        Self {
            reason: ProjectCatalogReadSourceKind::InvalidDeclarationFacts,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectCatalogReadError {
    #[error("project lifecycle changed during catalog query")]
    ProjectLifecycleChanged,
    #[error("catalog resource changed during catalog query")]
    CatalogResourceStale { resource: GraphResourceId },
    #[error("project lifecycle admission is closed")]
    AdmissionClosed,
    #[error("project recovery is required")]
    RecoveryRequired,
    #[error("project filesystem transaction is busy")]
    FilesystemBusy,
    #[error("project catalog facts could not be read")]
    ReadFailed(#[source] ProjectCatalogReadSource),
    #[error("project catalog invariant failed")]
    Internal(#[source] ProjectCatalogReadSource),
}

#[derive(Debug, thiserror::Error)]
pub enum GraphCatalogQueryError {
    #[error("graph revision changed during catalog query")]
    RevisionConflict {
        expected: GraphRevision,
        current: GraphRevision,
    },
    #[error("graph is not loaded")]
    GraphNotLoaded { graph: GraphResourcePath },
    #[error("compatible-catalog source port is invalid")]
    CompatibleSourceInvalid,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogQueryApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("captured catalog-query session changed")]
    SessionChanged,
    #[error("catalog project authority is stale")]
    CatalogProjectStale,
    #[error(transparent)]
    Project(#[from] ProjectCatalogReadError),
    #[error(transparent)]
    Database(#[from] yss_database_runtime::error::DatabaseError),
    #[error(transparent)]
    Contract(#[from] GraphContractMappingError),
    #[error(transparent)]
    Graph(#[from] GraphCatalogQueryError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogQueryResult {
    project_instance_id: ProjectInstanceId,
    registry_fingerprint: RegistryFingerprint,
    resource_publication_revision: u64,
    catalog: LocalizedCatalog,
}

impl CatalogQueryResult {
    fn new(
        project_instance_id: ProjectInstanceId,
        registry_fingerprint: RegistryFingerprint,
        resource_publication_revision: u64,
        catalog: LocalizedCatalog,
    ) -> Self {
        Self {
            project_instance_id,
            registry_fingerprint,
            resource_publication_revision,
            catalog,
        }
    }

    pub fn into_transport_parts(self) -> CatalogQueryResultParts {
        CatalogQueryResultParts {
            project_instance_id: self.project_instance_id,
            registry_fingerprint: self.registry_fingerprint,
            resource_publication_revision: self.resource_publication_revision,
            catalog: self.catalog,
        }
    }
}

pub struct CatalogQueryResultParts {
    project_instance_id: ProjectInstanceId,
    registry_fingerprint: RegistryFingerprint,
    resource_publication_revision: u64,
    catalog: LocalizedCatalog,
}

impl CatalogQueryResultParts {
    pub fn into_fields(
        self,
    ) -> (
        ProjectInstanceId,
        RegistryFingerprint,
        u64,
        LocalizedCatalog,
    ) {
        (
            self.project_instance_id,
            self.registry_fingerprint,
            self.resource_publication_revision,
            self.catalog,
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectCatalogResources {
    graph: ProjectGraphResourceSnapshot,
    entries: Box<[CatalogResourceEntry]>,
    database_observations: DatabaseDeclarationObservationSet,
}

impl ProjectCatalogResources {
    fn from_index(index: ProjectIndex) -> Result<Self, ProjectCatalogReadSource> {
        let authority_generation = index.authority_generation();
        let mut functions = BTreeMap::new();
        let mut variables = BTreeMap::new();
        let mut databases = BTreeMap::new();
        let mut entries = Vec::new();

        for graph in index.graphs {
            let Some(signature) = graph.function_signature else {
                continue;
            };
            let path = GraphResourcePath::new(graph.path.clone())
                .map_err(ProjectCatalogReadSource::invalid_graph_path)?;
            let signature = graph_signature(signature)
                .map_err(|_| ProjectCatalogReadSource::invalid_function_type())?;
            if functions.insert(path.clone(), signature).is_some() {
                return Err(ProjectCatalogReadSource::invalid_declaration_facts());
            }
            entries.push(CatalogResourceEntry {
                name: graph.name.into_boxed_str(),
                node_type_id: node_type("yssbi.project.function.call")?,
                resource_path: CatalogResourcePath::new(graph.path),
                resource_revision: graph.function_revision.unwrap_or(graph.revision).get(),
                create_args: ResourceBoundCreateArgs::Function,
                technical_terms: vec!["call".into(), "function".into()],
            });
        }

        for variable in index.variables {
            let resource = GraphResourceId::new(variable.resource_path.as_str());
            if variables
                .insert(resource, VariableValueContract::new(variable.data_type))
                .is_some()
            {
                return Err(ProjectCatalogReadSource::invalid_declaration_facts());
            }
            let get_node_type = node_type("yssbi.project.variable.get")?;
            let set_node_type = node_type("yssbi.project.variable.set")?;
            for node_type_id in [get_node_type, set_node_type] {
                entries.push(CatalogResourceEntry {
                    name: variable.name.clone().into_boxed_str(),
                    node_type_id,
                    resource_path: CatalogResourcePath::new(variable.resource_path.as_str()),
                    resource_revision: variable.revision.get(),
                    create_args: ResourceBoundCreateArgs::Variable,
                    technical_terms: vec!["variable".into()],
                });
            }
        }

        let mut declaration_observations = Vec::new();
        for database in index.databases {
            let id = DatabaseId::from_existing(database.id.clone().into());
            let declaration = DatabaseDecl {
                id: id.clone(),
                engine: database.engine,
                schema_version: database.schema_version,
                required: database.required,
                name: database
                    .name
                    .unwrap_or_else(|| database.id.clone())
                    .into_boxed_str(),
            };
            if databases.insert(id.clone(), declaration.clone()).is_some() {
                return Err(ProjectCatalogReadSource::invalid_declaration_facts());
            }
            declaration_observations.push((
                id,
                DatabaseDeclarationObservation::new(
                    DatabaseDeclarationRevision::from_existing(database.revision.get()),
                    DatabaseDeclarationFingerprint::from_decl(&declaration),
                ),
            ));
            entries.push(CatalogResourceEntry {
                name: declaration.name.clone(),
                node_type_id: node_type("yssbi.dataframe.source.get")?,
                resource_path: CatalogResourcePath::new(format!("databases/{}", database.id)),
                resource_revision: database.revision.get(),
                create_args: ResourceBoundCreateArgs::Database,
                technical_terms: vec!["dataframe".into(), "database".into()],
            });
        }

        let database_observations =
            DatabaseDeclarationObservationSet::try_from_iter(declaration_observations)
                .map_err(|_| ProjectCatalogReadSource::invalid_declaration_facts())?;
        let graph = ProjectGraphResourceSnapshot::new(
            ProjectInstanceId::from_existing(index.project_instance_id),
            authority_generation,
            functions,
            variables,
            databases,
        );
        Ok(Self {
            graph,
            entries: entries.into_boxed_slice(),
            database_observations,
        })
    }

    pub(crate) fn graph(&self) -> &ProjectGraphResourceSnapshot {
        &self.graph
    }

    pub(crate) fn entries(&self) -> &[CatalogResourceEntry] {
        &self.entries
    }

    pub(crate) fn database_observations(&self) -> &DatabaseDeclarationObservationSet {
        &self.database_observations
    }
}

#[derive(Clone, Debug)]
pub struct LocalizedCatalogProjectFacts {
    project_instance_id: ProjectInstanceId,
    authority_basis: ProjectAuthorityBasis,
    resource_publication_revision: u64,
    resources: ProjectCatalogResources,
}

#[derive(Clone, Debug)]
struct ProjectAuthorityBasis {
    project_instance_id: ProjectInstanceId,
    resource_publication_revision: u64,
    authority_generation: u64,
}

impl LocalizedCatalogProjectFacts {
    pub(crate) fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub(crate) fn resource_publication_revision(&self) -> u64 {
        self.resource_publication_revision
    }

    pub(crate) fn resources(&self) -> &ProjectCatalogResources {
        &self.resources
    }
}

#[derive(Clone, Debug)]
pub struct CompatibleCatalogProjectFacts {
    localized: LocalizedCatalogProjectFacts,
    graph: ResidentGraphCatalogFacts,
}

impl CompatibleCatalogProjectFacts {
    pub(crate) fn localized(&self) -> &LocalizedCatalogProjectFacts {
        &self.localized
    }

    pub(crate) fn resident_graph(&self) -> &ResidentGraphCatalogFacts {
        &self.graph
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ResidentGraphCatalogFacts {
    path: GraphResourcePath,
    revision: GraphRevision,
    document: Arc<GraphDocument>,
}

impl ResidentGraphCatalogFacts {
    pub(crate) fn path(&self) -> &GraphResourcePath {
        &self.path
    }

    pub(crate) const fn revision(&self) -> GraphRevision {
        self.revision
    }

    pub(crate) fn document(&self) -> &GraphDocument {
        &self.document
    }
}

pub(crate) fn capture_localized_project_facts(
    session: &ApplicationSession,
) -> Result<LocalizedCatalogProjectFacts, ProjectCatalogReadError> {
    let index = session
        .project()
        .read_project_index(session.project_instance_id())
        .map_err(map_project_catalog_error)?;
    if index.project_instance_id != session.project_instance_id().as_str() {
        return Err(ProjectCatalogReadError::ProjectLifecycleChanged);
    }
    let resource_publication_revision = index.publication_revision;
    let authority_generation = index.authority_generation();
    let project_instance_id = session.project_instance_id().clone();
    let resources =
        ProjectCatalogResources::from_index(index).map_err(ProjectCatalogReadError::Internal)?;
    Ok(LocalizedCatalogProjectFacts {
        project_instance_id: project_instance_id.clone(),
        authority_basis: ProjectAuthorityBasis {
            project_instance_id,
            resource_publication_revision,
            authority_generation,
        },
        resource_publication_revision,
        resources,
    })
}

pub(crate) fn capture_compatible_project_facts(
    session: &ApplicationSession,
    path: &GraphResourcePath,
    expected_revision: GraphRevision,
) -> Result<CompatibleCatalogProjectFacts, CatalogQueryApplicationError> {
    let localized = capture_localized_project_facts(session)?;
    let data = session
        .project()
        .get_data()
        .map_err(map_project_catalog_error)
        .map_err(CatalogQueryApplicationError::Project)?;
    revalidate_project_catalog_facts(session, &localized)?;
    let graph = data
        .graphs
        .get(path)
        .ok_or_else(|| GraphCatalogQueryError::GraphNotLoaded {
            graph: path.clone(),
        })?;
    if graph.document.revision != expected_revision {
        return Err(GraphCatalogQueryError::RevisionConflict {
            expected: expected_revision,
            current: graph.document.revision,
        }
        .into());
    }
    Ok(CompatibleCatalogProjectFacts {
        localized,
        graph: ResidentGraphCatalogFacts {
            path: path.clone(),
            revision: graph.document.revision,
            document: Arc::new(graph.document.clone()),
        },
    })
}

impl ApplicationState {
    pub fn localized_node_catalog(
        &self,
        request: LocalizedCatalogRequest,
    ) -> Result<CatalogQueryResult, CatalogQueryApplicationError> {
        let captured = self.capture_session()?;
        localized_node_catalog_in_session(self, &captured, request)
    }

    pub fn compatible_node_catalog(
        &self,
        request: CompatibleCatalogRequest,
    ) -> Result<CatalogQueryResult, CatalogQueryApplicationError> {
        let captured = self.capture_session()?;
        compatible_node_catalog_in_session(self, &captured, request)
    }
}

pub(crate) fn localized_node_catalog_in_session(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    request: LocalizedCatalogRequest,
) -> Result<CatalogQueryResult, CatalogQueryApplicationError> {
    ensure_requested_project(captured, request.project_instance_id())?;
    let project = capture_localized_project_facts(captured)?;
    let database = catalog_snapshot(captured.database())?;
    revalidate_declaration_observations(
        captured.database(),
        project.resources().database_observations(),
    )?;
    let _graph_catalog = build_resource_catalog(project.resources().graph(), &database)?;
    let localized = captured
        .graph()
        .localized_catalog_with_resources(project.resources().entries(), request.locale());

    revalidate_project_catalog_facts(captured, &project)?;
    revalidate_declaration_observations(
        captured.database(),
        project.resources().database_observations(),
    )?;
    revalidate_catalog_snapshot(captured.database(), &database)?;
    revalidate_application_session(application, captured)?;

    Ok(CatalogQueryResult::new(
        project.project_instance_id().clone(),
        RegistryFingerprint::from_bytes(captured.graph().registry_fingerprint()),
        project.resource_publication_revision(),
        localized,
    ))
}

pub(crate) fn compatible_node_catalog_in_session(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    request: CompatibleCatalogRequest,
) -> Result<CatalogQueryResult, CatalogQueryApplicationError> {
    ensure_requested_project(captured, request.project_instance_id())?;
    let project =
        capture_compatible_project_facts(captured, request.graph_path(), request.graph_revision())?;
    debug_assert_eq!(
        project.resident_graph().revision(),
        request.graph_revision()
    );
    let database = catalog_snapshot(captured.database())?;
    revalidate_declaration_observations(
        captured.database(),
        project.localized().resources().database_observations(),
    )?;
    let graph_catalog = build_resource_catalog(project.localized().resources().graph(), &database)?;
    let localized = captured
        .graph()
        .compatible_catalog_with_resources(
            project.resident_graph().path(),
            project.resident_graph().document(),
            request.source_port(),
            &graph_catalog,
            project.localized().resources().entries(),
            request.locale(),
        )
        .map_err(map_graph_catalog_error)?;

    revalidate_project_catalog_facts(captured, project.localized())?;
    revalidate_declaration_observations(
        captured.database(),
        project.localized().resources().database_observations(),
    )?;
    revalidate_catalog_snapshot(captured.database(), &database)?;
    revalidate_application_session(application, captured)?;

    Ok(CatalogQueryResult::new(
        project.localized().project_instance_id().clone(),
        RegistryFingerprint::from_bytes(captured.graph().registry_fingerprint()),
        project.localized().resource_publication_revision(),
        localized,
    ))
}

fn ensure_requested_project(
    captured: &ApplicationSession,
    requested: &ProjectInstanceId,
) -> Result<(), CatalogQueryApplicationError> {
    if requested != captured.project_instance_id() {
        return Err(CatalogQueryApplicationError::CatalogProjectStale);
    }
    Ok(())
}

fn revalidate_application_session(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
) -> Result<(), CatalogQueryApplicationError> {
    application
        .revalidate_captured_session(captured)
        .map_err(|error| match error {
            SessionRevalidationError::Unavailable(error) => {
                CatalogQueryApplicationError::SessionCapture(error)
            }
            SessionRevalidationError::Changed => CatalogQueryApplicationError::SessionChanged,
        })
}

pub(crate) fn revalidate_project_catalog_facts(
    session: &ApplicationSession,
    facts: &LocalizedCatalogProjectFacts,
) -> Result<(), ProjectCatalogReadError> {
    let current = session
        .project()
        .read_project_index(session.project_instance_id())
        .map_err(map_project_catalog_error)?;
    if current.project_instance_id != facts.authority_basis.project_instance_id.as_str()
        || current.publication_revision != facts.authority_basis.resource_publication_revision
        || current.authority_generation() != facts.authority_basis.authority_generation
    {
        return Err(ProjectCatalogReadError::CatalogResourceStale {
            resource: GraphResourceId::new("project/catalog"),
        });
    }
    Ok(())
}

fn map_project_catalog_error(error: ProjectFilesystemError) -> ProjectCatalogReadError {
    match error {
        ProjectFilesystemError::StaleProjectLifecycle { .. } => {
            ProjectCatalogReadError::ProjectLifecycleChanged
        }
        ProjectFilesystemError::CatalogResourceStale { .. } => {
            ProjectCatalogReadError::CatalogResourceStale {
                resource: GraphResourceId::new("project/catalog"),
            }
        }
        ProjectFilesystemError::ProjectLifecycleAdmissionClosed { .. } => {
            ProjectCatalogReadError::AdmissionClosed
        }
        ProjectFilesystemError::ProjectRecoveryRequired { .. } => {
            ProjectCatalogReadError::RecoveryRequired
        }
        ProjectFilesystemError::FilesystemTransactionBusy { .. } => {
            ProjectCatalogReadError::FilesystemBusy
        }
        error => ProjectCatalogReadError::ReadFailed(ProjectCatalogReadSource::filesystem(error)),
    }
}

fn map_graph_catalog_error(_: GraphRuntimeCatalogError) -> CatalogQueryApplicationError {
    GraphCatalogQueryError::CompatibleSourceInvalid.into()
}

fn graph_signature(
    signature: yss_project_history::FunctionSignature,
) -> Result<FunctionSignature, ()> {
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| parse_function_data_type(&parameter.type_name).map_err(|_| ()))
        .collect::<Result<Vec<_>, _>>()?;
    let result = signature
        .return_type
        .as_deref()
        .map(parse_function_data_type)
        .transpose()
        .map_err(|_| ())?;
    Ok(FunctionSignature::new(parameters, result))
}

fn node_type(
    value: &'static str,
) -> Result<yss_graph_protocol::NodeTypeId, ProjectCatalogReadSource> {
    yss_graph_protocol::NodeTypeId::new(value)
        .map_err(|_| ProjectCatalogReadSource::invalid_declaration_facts())
}

#[cfg(test)]
mod tests;
