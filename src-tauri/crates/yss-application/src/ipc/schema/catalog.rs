use crate::graph::catalog::CatalogQueryResult;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeCreationFormDto {
    pub values: yss_node_protocol::ParameterValues,
    pub groups: Vec<yss_ipc_contract::editor_projection::ParameterGroupDto>,
    pub port_counts: yss_node_protocol::InitialPortCounts,
    pub ports: Vec<NodeCreationPortDto>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeCreationPortDto {
    pub key: Box<str>,
    pub title: Box<str>,
    pub direction: &'static str,
    pub count: PortCountPolicyDto,
}

#[derive(Debug, serde::Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PortCountPolicyDto {
    Fixed,
    Configurable {
        min: u16,
        max: Option<u16>,
        member_templates: Box<[Box<str>]>,
    },
    Derived,
}

impl From<yss_node_catalog::NodeCreationPort> for NodeCreationPortDto {
    fn from(port: yss_node_catalog::NodeCreationPort) -> Self {
        Self {
            key: port.key.as_str().into(),
            title: port.title,
            direction: match port.direction {
                yss_node_protocol::PortDirection::Input => "input",
                yss_node_protocol::PortDirection::Output => "output",
            },
            count: match port.count {
                yss_node_catalog::PortCountPolicy::Fixed => PortCountPolicyDto::Fixed,
                yss_node_catalog::PortCountPolicy::Derived => PortCountPolicyDto::Derived,
                yss_node_catalog::PortCountPolicy::Configurable {
                    min,
                    max,
                    member_templates,
                } => PortCountPolicyDto::Configurable {
                    min,
                    max,
                    member_templates: member_templates
                        .iter()
                        .map(|key| key.as_str().into())
                        .collect(),
                },
            },
        }
    }
}

impl From<yss_graph_runtime::NodeCreationForm> for NodeCreationFormDto {
    fn from(form: yss_graph_runtime::NodeCreationForm) -> Self {
        Self {
            values: form.values,
            groups: form
                .groups
                .iter()
                .map(super::editor_projection::map_parameter_group)
                .collect(),
            port_counts: form.port_counts,
            ports: form.ports.into_vec().into_iter().map(Into::into).collect(),
        }
    }
}
use serde::Serialize;
use yss_node_catalog::{
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
    pub available: bool,
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
    #[serde(rename = "function_graph")]
    FunctionGraph,
    Database,
}

impl From<yss_node_catalog::LocalizedCatalog> for LocalizedCatalogDto {
    fn from(catalog: yss_node_catalog::LocalizedCatalog) -> Self {
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

impl From<yss_node_catalog::LocalizedCategory> for LocalizedCategoryDto {
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

impl From<yss_node_catalog::LocalizedCatalogItem> for LocalizedCatalogItemDto {
    fn from(item: DomainCatalogItem) -> Self {
        Self {
            available: item.available,
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
                .map(|path: yss_node_catalog::CatalogResourcePath| path.as_str().into()),
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
                    .map(|parameter: yss_node_protocol::ParameterKey| parameter.as_str().into())
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
            DomainCreateArgs::FunctionGraph => Self::FunctionGraph,
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

impl TryFrom<NodeCreationDescriptorDto> for yss_node_catalog::NodeCreation {
    type Error = NodeCreationMappingError;

    fn try_from(value: NodeCreationDescriptorDto) -> Result<Self, Self::Error> {
        let node_type = |value: Box<str>| {
            yss_node_protocol::NodeTypeId::new(value)
                .map_err(|_| NodeCreationMappingError::InvalidNodeType)
        };
        let parameter = |value: Box<str>| {
            yss_node_protocol::ParameterKey::new(value)
                .map_err(|_| NodeCreationMappingError::InvalidParameter)
        };
        Ok(match value {
            NodeCreationDescriptorDto::Static { node_type_id } => {
                yss_node_catalog::NodeCreation::Static {
                    node_type_id: node_type(node_type_id)?,
                }
            }
            NodeCreationDescriptorDto::ParameterizedStatic {
                node_type_id,
                required_parameters,
            } => yss_node_catalog::NodeCreation::ParameterizedStatic {
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
            } => yss_node_catalog::NodeCreation::ResourceBound {
                node_type_id: node_type(node_type_id)?,
                resource_path: yss_node_catalog::CatalogResourcePath::new(resource_path),
                resource_revision,
                create_args: match create_args {
                    ResourceBoundCreateArgsDto::FunctionGraph => {
                        yss_node_catalog::ResourceBoundCreateArgs::FunctionGraph
                    }
                    ResourceBoundCreateArgsDto::Database => {
                        yss_node_catalog::ResourceBoundCreateArgs::Database
                    }
                },
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::catalog::LocalizedCatalogRequest;
    use crate::session::{ApplicationSession, ApplicationSessionEpoch, ApplicationSessionSlot};
    use std::num::NonZeroU64;
    use std::sync::Arc;
    use yss_database_contract::{
        DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet,
        DatabaseId, DatabaseSessionIdentity, DatabaseSessionOpenRequest,
    };
    use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
    use yss_graph_document::GraphResourceKind;
    use yss_graph_execution::identity::{ExecutionSessionId, RuntimeGeneration};
    use yss_graph_execution::resource_preparation::ResourceProviderFactory;
    use yss_graph_execution::state::ExecutionRuntimeState;
    use yss_graph_runtime::{GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState};
    use yss_node_catalog::build_builtin_node_system;
    use yss_project_identity::ProjectSessionId;
    use yss_project_model::ProjectData;

    fn application_with_function() -> (
        yss_project::fixtures::TempProject,
        crate::session::ApplicationState,
    ) {
        let path =
            yss_graph_document::GraphResourcePath::new("functions/Opaque.yssbi-function").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            path.clone(),
            yss_project_model::GraphResourceDocument::new(
                "Opaque Function",
                GraphResourceKind::FunctionGraph,
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
        let graph = Arc::new(
            GraphRuntimeState::from_components(
                GraphRuntimeEpoch::from_existing(1),
                GraphRuntimeComponents {
                    registry: builtin.registry,
                    catalog: builtin.catalog,
                },
            )
            .unwrap(),
        );
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
                    Vec::<DatabaseDecl>::new().into(),
                    observations,
                ))
                .unwrap(),
        );
        let execution = Arc::new(ExecutionRuntimeState::new(
            execution_session_id,
            RuntimeGeneration::from_existing(1),
            yss_node_kernel::KernelRegistry::default().into(),
            yss_database_runtime::dataset_query_engine().unwrap(),
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
        let slot = Arc::new(ApplicationSessionSlot::new(
            crate::session::NodeComponents::builtins().unwrap(),
        ));
        slot.publish_for_test(session);
        (fixture, crate::session::ApplicationState::new(slot))
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

        assert!(!wire["projectInstanceId"].as_str().unwrap().is_empty());
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
        assert_eq!(item["available"], true);
        let builtin_availability = [
            ("yssbi.statistics.logit.fit", true),
            ("yssbi.statistics.inequality.gini", true),
            ("yssbi.statistics.postestimation.adjusted_predictions", true),
        ];
        for (id, expected) in builtin_availability {
            let node = wire["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["nodeTypeId"] == id)
                .unwrap();
            assert_eq!(node["available"], expected);
        }
        assert_eq!(item["creation"]["kind"], "resourceBound");
        assert_eq!(item["creation"]["createArgs"]["kind"], "function_graph");
        assert!(
            item["ports"]
                .as_array()
                .is_some_and(|ports| ports.iter().all(|port| port.get("kind").is_none()))
        );
        assert!(wire.get("project_instance_id").is_none());
        assert!(wire.get("registry_fingerprint").is_none());
        assert!(wire.get("resource_publication_revision").is_none());

        // The same authorities supply Activity JSON, including opaque resource descriptors.
        let project_id = application
            .capture_session()
            .unwrap()
            .project_instance_id()
            .clone();
        let activity = application
            .nodes_activity_panel(Some(project_id.clone()), "zh-CN".into())
            .unwrap();
        let activity = serde_json::to_value(
            crate::ipc::schema::activity_panel::ActivityPanelDocumentDto::try_from(activity)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(activity["schema"], "yssbi.activity-panel.v1");
        assert_eq!(activity["projectInstanceId"], project_id.as_str());
        let rows = activity["rows"].as_array().unwrap();
        let function = rows
            .iter()
            .find(|row| {
                row["item"]["creation"]["resourcePath"] == "functions/Opaque.yssbi-function"
            })
            .unwrap();
        assert_eq!(function["item"]["creation"], item["creation"]);
        assert_eq!(function["item"]["available"], true);
        for (id, expected) in builtin_availability {
            assert!(
                rows.iter()
                    .any(|row| row["item"]["creation"]["nodeTypeId"] == id
                        && row["item"]["available"] == expected)
            );
        }
        assert!(rows.iter().any(|row| row["kind"] == "category"));
        let form = application
            .node_creation_form(
                &project_id,
                &"yssbi.statistics.linear.fit".parse().unwrap(),
                Default::default(),
                [("x".parse().unwrap(), 4)].into(),
                "zh-CN",
            )
            .unwrap();
        let wire = serde_json::to_value(NodeCreationFormDto::from(form)).unwrap();
        assert_eq!(wire["portCounts"]["x"], 4);
        assert!(wire["groups"].is_array());
        let x = wire["ports"]
            .as_array()
            .unwrap()
            .iter()
            .find(|port| port["key"] == "x")
            .unwrap();
        assert_eq!(x["count"]["kind"], "configurable");
        assert_eq!(x["count"]["memberTemplates"], serde_json::json!(["x"]));
    }
}
