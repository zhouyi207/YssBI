use serde::Serialize;
use yss_application::catalog_query::CatalogQueryResult;
use yss_graph_catalog::{
    LocalizedCatalogItem as DomainCatalogItem, LocalizedCategory as DomainCategory,
    LocalizedParameter as DomainParameter, LocalizedPort as DomainPort,
    NodeCreation as DomainCreationDescriptor, ResourceBoundCreateArgs as DomainCreateArgs,
};

/// The catalog wire shape is owned by the transport schema layer.
///
/// Graph supplies the transport-neutral projection; this conversion is the
/// only boundary that adds project/session metadata to the catalog wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedCatalogDto {
    pub project_instance_id: Box<str>,
    pub registry_fingerprint: Box<str>,
    pub resource_publication_revision: u64,
    pub locale: Box<str>,
    pub categories: Vec<LocalizedCategoryDto>,
    pub items: Vec<LocalizedCatalogItemDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedCategoryDto {
    pub category_id: Box<str>,
    pub parent_category_id: Option<Box<str>>,
    pub order: i32,
    pub title: Box<str>,
    pub search_text: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedCatalogItemDto {
    pub node_type_id: Box<str>,
    pub title: Box<str>,
    pub documentation: Option<Box<str>>,
    pub category_id: Box<str>,
    pub icon_id: Box<str>,
    pub style_id: Box<str>,
    pub aliases: Vec<Box<str>>,
    pub technical_terms: Vec<Box<str>>,
    pub backend_search_text: Vec<Box<str>>,
    pub resource_names: Vec<Box<str>>,
    pub ports: Vec<LocalizedPortDto>,
    pub parameters: Vec<LocalizedParameterDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_path: Option<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_revision: Option<u64>,
    pub creation: NodeCreationDescriptorDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedPortDto {
    pub key: Box<str>,
    pub label: Box<str>,
    pub direction: Box<str>,
    pub kind: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedParameterDto {
    pub key: Box<str>,
    pub title: Box<str>,
    pub description: Option<Box<str>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum NodeCreationDescriptorDto {
    #[serde(rename = "static")]
    Static {
        #[serde(rename = "nodeTypeId")]
        node_type_id: Box<str>,
    },
    #[serde(rename = "parameterizedStatic")]
    ParameterizedStatic {
        #[serde(rename = "nodeTypeId")]
        node_type_id: Box<str>,
        #[serde(rename = "requiredParameters")]
        required_parameters: Box<[Box<str>]>,
    },
    #[serde(rename = "resourceBound")]
    ResourceBound {
        #[serde(rename = "nodeTypeId")]
        node_type_id: Box<str>,
        #[serde(rename = "resourcePath")]
        resource_path: Box<str>,
        #[serde(rename = "resourceRevision")]
        resource_revision: u64,
        #[serde(rename = "createArgs")]
        create_args: ResourceBoundCreateArgsDto,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResourceBoundCreateArgsDto {
    Function,
    Variable,
    Database,
}

impl From<yss_graph_catalog::LocalizedCatalog> for LocalizedCatalogDto {
    fn from(catalog: yss_graph_catalog::LocalizedCatalog) -> Self {
        Self {
            project_instance_id: Box::default(),
            registry_fingerprint: Box::default(),
            resource_publication_revision: 0,
            locale: catalog.locale,
            categories: catalog
                .categories
                .into_iter()
                .map(LocalizedCategoryDto::from)
                .collect(),
            items: catalog
                .items
                .into_iter()
                .map(LocalizedCatalogItemDto::from)
                .collect(),
        }
    }
}

impl From<CatalogQueryResult> for LocalizedCatalogDto {
    fn from(result: CatalogQueryResult) -> Self {
        let (project_instance_id, registry_fingerprint, resource_publication_revision, catalog) =
            result.into_transport_parts().into_fields();
        let mut mapped = Self::from(catalog);
        mapped.project_instance_id = project_instance_id.as_str().into();
        mapped.registry_fingerprint = registry_fingerprint.to_hex().into();
        mapped.resource_publication_revision = resource_publication_revision;
        mapped
    }
}

impl From<yss_graph_catalog::LocalizedCategory> for LocalizedCategoryDto {
    fn from(category: DomainCategory) -> Self {
        Self {
            category_id: category.category_id,
            parent_category_id: category.parent_category_id,
            order: category.order,
            title: category.title,
            search_text: category.search_text,
        }
    }
}

impl From<yss_graph_catalog::LocalizedCatalogItem> for LocalizedCatalogItemDto {
    fn from(item: DomainCatalogItem) -> Self {
        Self {
            node_type_id: item.node_type_id,
            title: item.title,
            documentation: item.documentation,
            category_id: item.category_id,
            icon_id: item.icon_id,
            style_id: item.style_id,
            aliases: item.aliases,
            technical_terms: item.technical_terms,
            backend_search_text: item.backend_search_text,
            resource_names: item.resource_names,
            ports: item.ports.into_iter().map(LocalizedPortDto::from).collect(),
            parameters: item
                .parameters
                .into_iter()
                .map(LocalizedParameterDto::from)
                .collect(),
            resource_path: item
                .resource_path
                .map(|path: yss_graph_catalog::CatalogResourcePath| path.as_str().into()),
            resource_revision: item.resource_revision,
            creation: item.creation.into(),
        }
    }
}

impl From<DomainPort> for LocalizedPortDto {
    fn from(port: DomainPort) -> Self {
        Self {
            key: port.key,
            label: port.label,
            direction: port.direction,
            kind: port.kind,
        }
    }
}

impl From<DomainParameter> for LocalizedParameterDto {
    fn from(parameter: DomainParameter) -> Self {
        Self {
            key: parameter.key,
            title: parameter.title,
            description: parameter.description,
        }
    }
}

impl From<DomainCreationDescriptor> for NodeCreationDescriptorDto {
    fn from(descriptor: DomainCreationDescriptor) -> Self {
        match descriptor {
            DomainCreationDescriptor::Static { node_type_id } => Self::Static {
                node_type_id: node_type_id.as_str().into(),
            },
            DomainCreationDescriptor::ParameterizedStatic {
                node_type_id,
                required_parameters,
            } => Self::ParameterizedStatic {
                node_type_id: node_type_id.as_str().into(),
                required_parameters: required_parameters
                    .into_iter()
                    .map(|parameter: yss_graph_protocol::ParameterKey| parameter.as_str().into())
                    .collect(),
            },
            DomainCreationDescriptor::ResourceBound {
                node_type_id,
                resource_path,
                resource_revision,
                create_args,
            } => Self::ResourceBound {
                node_type_id: node_type_id.as_str().into(),
                resource_path: resource_path.as_str().into(),
                resource_revision,
                create_args: create_args.into(),
            },
        }
    }
}

impl From<DomainCreateArgs> for ResourceBoundCreateArgsDto {
    fn from(value: DomainCreateArgs) -> Self {
        match value {
            DomainCreateArgs::Function => Self::Function,
            DomainCreateArgs::Variable => Self::Variable,
            DomainCreateArgs::Database => Self::Database,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NodeCreationMappingError {
    #[error("node creation descriptor contains an invalid node type")]
    InvalidNodeType,
    #[error("node creation descriptor contains an invalid required parameter")]
    InvalidParameter,
}

impl TryFrom<NodeCreationDescriptorDto> for yss_graph_catalog::NodeCreation {
    type Error = NodeCreationMappingError;

    fn try_from(value: NodeCreationDescriptorDto) -> Result<Self, Self::Error> {
        let node_type = |value: Box<str>| {
            yss_graph_protocol::NodeTypeId::new(value)
                .map_err(|_| NodeCreationMappingError::InvalidNodeType)
        };
        let parameter = |value: Box<str>| {
            yss_graph_protocol::ParameterKey::new(value)
                .map_err(|_| NodeCreationMappingError::InvalidParameter)
        };
        Ok(match value {
            NodeCreationDescriptorDto::Static { node_type_id } => {
                yss_graph_catalog::NodeCreation::Static {
                    node_type_id: node_type(node_type_id)?,
                }
            }
            NodeCreationDescriptorDto::ParameterizedStatic {
                node_type_id,
                required_parameters,
            } => yss_graph_catalog::NodeCreation::ParameterizedStatic {
                node_type_id: node_type(node_type_id)?,
                required_parameters: required_parameters
                    .into_vec()
                    .into_iter()
                    .map(parameter)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            },
            NodeCreationDescriptorDto::ResourceBound {
                node_type_id,
                resource_path,
                resource_revision,
                create_args,
            } => yss_graph_catalog::NodeCreation::ResourceBound {
                node_type_id: node_type(node_type_id)?,
                resource_path: yss_graph_catalog::CatalogResourcePath::new(resource_path),
                resource_revision,
                create_args: match create_args {
                    ResourceBoundCreateArgsDto::Function => {
                        yss_graph_catalog::ResourceBoundCreateArgs::Function
                    }
                    ResourceBoundCreateArgsDto::Variable => {
                        yss_graph_catalog::ResourceBoundCreateArgs::Variable
                    }
                    ResourceBoundCreateArgsDto::Database => {
                        yss_graph_catalog::ResourceBoundCreateArgs::Database
                    }
                },
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU64;
    use std::sync::Arc;
    use yss_application::catalog_query::LocalizedCatalogRequest;
    use yss_application::execution::{
        ApplicationSession, ApplicationSessionEpoch, ApplicationSessionSlot,
    };
    use yss_database_contract::{
        DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet,
        DatabaseId, DatabaseSessionIdentity, DatabaseSessionOpenRequest,
    };
    use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
    use yss_execution::identity::{ExecutionSessionId, RuntimeGeneration};
    use yss_execution::resource_preparation::ResourceProviderFactory;
    use yss_execution::state::ExecutionRuntimeState;
    use yss_graph_catalog::build_builtin_node_system;
    use yss_graph_document::GraphResourceKind;
    use yss_graph_runtime::{GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState};
    use yss_project_identity::ProjectSessionId;
    use yss_project_model::ProjectData;

    fn application_with_function() -> (
        yss_project::fixtures::TempProject,
        yss_application::execution::ApplicationState,
    ) {
        let path =
            yss_graph_document::GraphResourcePath::new("functions/Opaque.yssbi-function").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            path.clone(),
            yss_project_model::GraphResourceDocument::new(
                "Opaque Function",
                GraphResourceKind::Function,
            ),
        );
        let fixture =
            yss_project::fixtures::TempProject::activate("catalog-schema-mapper", project.clone());
        let root = fixture.state().get_path().unwrap();
        yss_project::fixtures::write_graph(&project, &root, &path).unwrap();
        let project = Arc::new(fixture.state().clone());
        let project_instance_id = project.capture_project_session().unwrap().instance_id;
        let project_session_id = ProjectSessionId::new("catalog-schema-session");
        let execution_session_id = ExecutionSessionId::new(uuid::Uuid::new_v4());
        let builtin = build_builtin_node_system().unwrap();
        let graph = Arc::new(GraphRuntimeState::from_components(
            GraphRuntimeEpoch::from_existing(1),
            GraphRuntimeComponents {
                registry: builtin.registry,
                catalog: builtin.catalog,
            },
        ));
        let observations = DatabaseDeclarationObservationSet::try_from_iter(std::iter::empty::<(
            DatabaseId,
            DatabaseDeclarationObservation,
        )>())
        .unwrap();
        let database = Arc::new(
            DatabaseRuntimeRegistry::new()
                .open_session(DatabaseSessionOpenRequest::new(
                    DatabaseSessionIdentity::from_existing(project_session_id.as_str().into()),
                    NonZeroU64::new(1).unwrap(),
                    None,
                    Vec::<DatabaseDecl>::new().into(),
                    observations,
                ))
                .unwrap(),
        );
        let execution = Arc::new(ExecutionRuntimeState::new(
            execution_session_id,
            RuntimeGeneration::from_existing(1),
        ));
        let session = Arc::new(ApplicationSession::new_for_test(
            ApplicationSessionEpoch::from_existing(1),
            project_instance_id,
            project_session_id.clone(),
            execution_session_id,
            RuntimeGeneration::from_existing(1),
            project,
            graph,
            execution,
            database,
            Arc::new(ResourceProviderFactory::new(
                project_session_id.as_str().into(),
            )),
        ));
        let slot = Arc::new(ApplicationSessionSlot::new());
        slot.publish_for_test(session);
        (
            fixture,
            yss_application::execution::ApplicationState::new(slot),
        )
    }

    #[test]
    fn catalog_mapper_preserves_metadata_and_resource_bound_wire_shape() {
        let (_fixture, application) = application_with_function();
        let project_instance_id = application
            .capture_session()
            .unwrap()
            .project_instance_id()
            .clone();
        let result = application
            .localized_node_catalog(LocalizedCatalogRequest::new(project_instance_id, "zh-CN"))
            .unwrap();
        let wire = serde_json::to_value(LocalizedCatalogDto::from(result)).unwrap();

        assert_eq!(
            wire["projectInstanceId"].as_str().unwrap().is_empty(),
            false
        );
        assert!(!wire["registryFingerprint"].as_str().unwrap().is_empty());
        assert_eq!(wire["resourcePublicationRevision"], 0);
        assert_eq!(wire["locale"], "zh-CN");
        let item = wire["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["resourcePath"] == "functions/Opaque.yssbi-function")
            .expect("function resource item is present");
        assert_eq!(item["resourceRevision"], 0);
        assert_eq!(item["creation"]["kind"], "resourceBound");
        assert_eq!(item["creation"]["createArgs"]["kind"], "function");
        assert!(wire.get("project_instance_id").is_none());
        assert!(wire.get("registry_fingerprint").is_none());
        assert!(wire.get("resource_publication_revision").is_none());
    }
}
