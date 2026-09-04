use crate::mutation::{EditorMutationError, EditorMutationErrorCode};
use std::collections::{BTreeMap, BTreeSet};
use yss_data_contract::DataType;
use yss_graph_catalog::{
    CatalogResourceEntry, CatalogResourcePath, LocalizedCatalog, NodeCreation,
    ResourceBoundCreateArgs,
};
use yss_graph_document::{
    DocumentNode, DynamicMemberLocator, DynamicPortBinding, FunctionParameterId, GraphDocument,
    GraphResourceKind, GraphResourcePath, LastKnownPortMetadata, OrderKey, PortAddress, PortRef,
};
use yss_graph_protocol::{
    ConnectionsPerPort, NodeInstanceDisplaySpec, NodeProtocol, ParameterKey, PortCardinality,
    PortDirection, PortKey, PortSpec, ResourceDisplayKind, TypeExpr, TypeParameterId,
};
use yss_graph_registry::NodeRegistry;
use yss_graph_resource_contract::{GraphResourceId, ResourceCatalogSnapshot};
use yss_variable_contract::VariableScope;

#[derive(Clone, Debug, PartialEq)]
/// Editor-facing catalog authority used to validate creation descriptors and dynamic ports.
///
/// This is intentionally richer than `yss_graph_resource_contract::ResourceCatalogSnapshot`,
/// which owns only the type/schema facts required by graph compilation. Project/session
/// currentness remains enforced by the caller's graph-operation commit authority.
pub struct CatalogMutationValidationSnapshot {
    pub resources: BTreeMap<CatalogResourcePath, CatalogMutationResource>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogCompatibilityError {
    SourceInvalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogFunctionParameter {
    pub id: yss_graph_document::FunctionParameterId,
    pub name: String,
    pub type_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogFunctionSignature {
    pub parameters: Box<[CatalogFunctionParameter]>,
    pub return_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CatalogMutationResource {
    Function {
        revision: u64,
        signature: CatalogFunctionSignature,
    },
    Variable {
        revision: u64,
        scope: VariableScope,
        data_type: DataType,
    },
    Database {
        authority_revision: u64,
    },
}

impl CatalogMutationResource {
    pub(crate) fn create_args(&self) -> ResourceBoundCreateArgs {
        match self {
            Self::Function { .. } => ResourceBoundCreateArgs::Function,
            Self::Variable { .. } => ResourceBoundCreateArgs::Variable,
            Self::Database { .. } => ResourceBoundCreateArgs::Database,
        }
    }

    pub(crate) fn display_kind(&self) -> ResourceDisplayKind {
        resource_display_kind(self.create_args())
    }

    pub(crate) fn revision(&self) -> u64 {
        match self {
            Self::Function { revision, .. } | Self::Variable { revision, .. } => *revision,
            Self::Database {
                authority_revision, ..
            } => *authority_revision,
        }
    }

    pub(crate) fn variable_scope(&self) -> Option<&VariableScope> {
        match self {
            Self::Variable { scope, .. } => Some(scope),
            Self::Function { .. } | Self::Database { .. } => None,
        }
    }
}

fn resource_display_kind(create_args: ResourceBoundCreateArgs) -> ResourceDisplayKind {
    match create_args {
        ResourceBoundCreateArgs::Function => ResourceDisplayKind::Function,
        ResourceBoundCreateArgs::Variable => ResourceDisplayKind::Variable,
        ResourceBoundCreateArgs::Database => ResourceDisplayKind::Database,
    }
}

pub(crate) fn resource_path_is_valid(
    resource_path: &CatalogResourcePath,
    create_args: ResourceBoundCreateArgs,
) -> bool {
    let path = resource_path.as_str();
    match create_args {
        ResourceBoundCreateArgs::Function => GraphResourcePath::new(path).is_ok_and(|canonical| {
            canonical.as_str() == path && canonical.as_str().starts_with("functions/")
        }),
        ResourceBoundCreateArgs::Variable => path
            .strip_prefix("variables/")
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
            .is_some_and(|id| format!("variables/{id}") == path),
        ResourceBoundCreateArgs::Database => path
            .strip_prefix("databases/")
            .is_some_and(|id| !id.is_empty()),
    }
}

pub(crate) fn variable_in_scope(graph_path: &GraphResourcePath, scope: &VariableScope) -> bool {
    match scope {
        VariableScope::Global => true,
        VariableScope::Event { event_path } => event_path.as_str() == graph_path.as_str(),
        VariableScope::Function { function_path } => function_path.as_str() == graph_path.as_str(),
    }
}

pub(crate) fn resource_parameter(
    protocol: &NodeProtocol,
    create_args: ResourceBoundCreateArgs,
) -> Result<&ParameterKey, String> {
    let NodeInstanceDisplaySpec::ResourceParameter { parameter, kind } = &protocol.instance_display
    else {
        return Err(format!(
            "node type '{}' is not resource-bound",
            protocol.type_id
        ));
    };
    if *kind != resource_display_kind(create_args) {
        return Err(format!(
            "node type '{}' resource kind does not match catalog authority",
            protocol.type_id
        ));
    }
    Ok(parameter)
}

fn mutation_validation_error(
    code: EditorMutationErrorCode,
    detail: impl Into<Box<str>>,
) -> EditorMutationError {
    EditorMutationError {
        code,
        detail: detail.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourcePort {
    pub address: PortAddress,
    pub direction: PortDirection,
    pub value_type: TypeExpr,
    pub type_parameters: Box<[TypeParameterId]>,
}

#[derive(Debug)]
pub(crate) struct ResolvedEditorPort<'a> {
    pub spec: &'a PortSpec,
    pub binding: Option<&'a DynamicPortBinding>,
    pub protocol: &'a NodeProtocol,
}

pub(crate) fn resolve_editor_port<'a>(
    document: &'a GraphDocument,
    registry: &'a NodeRegistry,
    address: &PortAddress,
) -> Result<ResolvedEditorPort<'a>, EditorMutationError> {
    let node = document.nodes.get(&address.node_id).ok_or_else(|| {
        mutation_validation_error(
            EditorMutationErrorCode::GraphPortNotFound,
            format!("endpoint node '{}' does not exist", address.node_id),
        )
    })?;
    let protocol = registry.protocol(&node.node_type).ok_or_else(|| {
        mutation_validation_error(
            EditorMutationErrorCode::GraphPortNotFound,
            format!("unknown node type '{}'", node.node_type),
        )
    })?;
    let template = match &address.port {
        PortRef::Declared { key } => key,
        PortRef::Instance { template, .. } => template,
    };
    let spec = protocol
        .interface
        .ports
        .iter()
        .find(|spec| &spec.key == template)
        .ok_or_else(|| {
            mutation_validation_error(
                EditorMutationErrorCode::GraphPortNotFound,
                format!("unknown port '{address}'"),
            )
        })?;
    let binding = match &address.port {
        PortRef::Declared { .. } if matches!(spec.cardinality, PortCardinality::Declared) => None,
        PortRef::Declared { .. } => {
            return Err(mutation_validation_error(
                EditorMutationErrorCode::GraphPortNotFound,
                format!("port '{address}' requires an instance address"),
            ));
        }
        PortRef::Instance { .. } => {
            let binding = document.port_bindings.get(address).ok_or_else(|| {
                mutation_validation_error(
                    EditorMutationErrorCode::GraphPortNotFound,
                    format!("instance port '{address}' has no binding"),
                )
            })?;
            let compatible = matches!(
                (&spec.cardinality, binding),
                (
                    PortCardinality::UserCreated { .. },
                    DynamicPortBinding::UserCreated { .. }
                ) | (
                    PortCardinality::Derived { .. },
                    DynamicPortBinding::Resolved { .. } | DynamicPortBinding::Orphan { .. }
                )
            );
            if !compatible {
                return Err(mutation_validation_error(
                    EditorMutationErrorCode::GraphPortNotFound,
                    format!("port binding kind does not match template '{address}'"),
                ));
            }
            Some(binding)
        }
    };
    Ok(ResolvedEditorPort {
        spec,
        binding,
        protocol,
    })
}

pub(crate) fn source_port(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &CatalogMutationValidationSnapshot,
    address: PortAddress,
) -> Result<SourcePort, EditorMutationError> {
    source_port_with_optional_catalog(document, registry, Some(resources), address)
}

pub(crate) fn validate_connection_types(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: Option<&CatalogMutationValidationSnapshot>,
    output: &PortAddress,
    input: &PortAddress,
) -> Result<(), EditorMutationError> {
    let output = source_port_with_optional_catalog(document, registry, resources, output.clone())?;
    let input = source_port_with_optional_catalog(document, registry, resources, input.clone())?;
    if output.direction != PortDirection::Output || input.direction != PortDirection::Input {
        return Err(mutation_validation_error(
            EditorMutationErrorCode::GraphConnectionDirectionMismatch,
            "connection endpoints have invalid directions",
        ));
    }
    if !type_pattern_is_exact(&output.value_type) {
        return Ok(());
    }
    if yss_graph_protocol::type_exprs_compatibility(
        &output.value_type,
        &input.value_type,
        &output.type_parameters,
        &input.type_parameters,
    ) != yss_graph_protocol::TypeCompatibility::Incompatible
    {
        Ok(())
    } else {
        Err(mutation_validation_error(
            EditorMutationErrorCode::GraphConnectionTypeMismatch,
            "connection endpoint types are not assignable",
        ))
    }
}

fn type_pattern_is_exact(value: &TypeExpr) -> bool {
    match value {
        TypeExpr::Concrete(_) => true,
        TypeExpr::Applied { arguments, .. } => arguments.iter().all(type_pattern_is_exact),
        TypeExpr::Class(_) | TypeExpr::Generic(_) | TypeExpr::Union(_) | TypeExpr::Unknown => false,
    }
}

fn source_port_with_optional_catalog(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: Option<&CatalogMutationValidationSnapshot>,
    address: PortAddress,
) -> Result<SourcePort, EditorMutationError> {
    let resolved = resolve_editor_port(document, registry, &address)?;
    let ResolvedEditorPort {
        spec,
        binding,
        protocol,
    } = resolved;
    if matches!(binding, Some(DynamicPortBinding::Orphan { .. })) {
        return Err(mutation_validation_error(
            EditorMutationErrorCode::GraphPortOrphan,
            "orphan ports cannot be connected",
        ));
    }
    let mut source = SourcePort {
        address,
        direction: spec.direction,
        value_type: spec.value_type.clone(),
        type_parameters: protocol.interface.type_parameters.clone(),
    };
    if let Some(resources) = resources {
        refine_source_type(&mut source, document, protocol, resources)?;
    }
    Ok(source)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidatePort {
    pub template: PortKey,
    pub direction: PortDirection,
    pub connections: ConnectionsPerPort,
    pub value_type: TypeExpr,
    pub type_parameters: Box<[TypeParameterId]>,
    pub dynamic: Option<DynamicCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicCandidate {
    pub origin: DynamicMemberLocator,
    pub order: OrderKey,
    pub last_known: LastKnownPortMetadata,
}

pub(crate) fn refine_source_type(
    source: &mut SourcePort,
    document: &GraphDocument,
    protocol: &NodeProtocol,
    resources: &CatalogMutationValidationSnapshot,
) -> Result<(), EditorMutationError> {
    let Some(node) = document.nodes.get(&source.address.node_id) else {
        return Ok(());
    };
    let Some((resource_path, resource)) = bound_catalog_resource(node, protocol, resources)? else {
        return Ok(());
    };
    match resource {
        CatalogMutationResource::Variable { data_type, .. } => {
            source.value_type = editor_type_expr(data_type).map_err(|error| {
                connection_type_unavailable(format!(
                    "variable resource '{}' has an invalid authoritative type: {error}",
                    resource_path.as_str()
                ))
            })?;
        }
        CatalogMutationResource::Function { signature, .. } => {
            let binding = document.port_bindings.get(&source.address).ok_or_else(|| {
                connection_type_unavailable(format!(
                    "function port '{}' has no authoritative member binding",
                    source.address
                ))
            })?;
            let origin = match binding {
                yss_graph_document::DynamicPortBinding::Resolved { origin, .. }
                | yss_graph_document::DynamicPortBinding::Orphan { origin, .. } => origin,
                yss_graph_document::DynamicPortBinding::UserCreated { .. } => {
                    return Err(connection_type_unavailable(format!(
                        "function port '{}' has a non-authoritative member binding",
                        source.address
                    )));
                }
            };
            let DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } = origin
            else {
                return Err(connection_type_unavailable(format!(
                    "function port '{}' has the wrong member origin",
                    source.address
                )));
            };
            if function.as_str() != resource_path.as_str() {
                return Err(connection_type_unavailable(format!(
                    "function port '{}' member origin does not match its bound resource",
                    source.address
                )));
            }
            let type_name = if parameter.as_str() == "return" {
                signature.return_type.as_deref()
            } else {
                signature
                    .parameters
                    .iter()
                    .find(|candidate| candidate.id == *parameter)
                    .map(|candidate| candidate.type_name.as_str())
            }
            .ok_or_else(|| {
                connection_type_unavailable(format!(
                    "function resource '{}' has no authoritative type for member '{}'",
                    resource_path.as_str(),
                    parameter.as_str()
                ))
            })?;
            source.value_type = function_type_expr(type_name).map_err(|error| {
                connection_type_unavailable(format!(
                    "function resource '{}' member '{}' has an invalid authoritative type: {error}",
                    resource_path.as_str(),
                    parameter.as_str()
                ))
            })?;
        }
        CatalogMutationResource::Database { .. } => {
            source.value_type = editor_type_expr(&DataType::DataFrame).map_err(|error| {
                connection_type_unavailable(format!(
                    "database resource '{}' has an invalid authoritative type: {error}",
                    resource_path.as_str()
                ))
            })?;
        }
    }
    Ok(())
}

pub(crate) fn bound_catalog_resource<'a>(
    node: &DocumentNode,
    protocol: &NodeProtocol,
    resources: &'a CatalogMutationValidationSnapshot,
) -> Result<Option<(&'a CatalogResourcePath, &'a CatalogMutationResource)>, EditorMutationError> {
    let NodeInstanceDisplaySpec::ResourceParameter { parameter, kind } = &protocol.instance_display
    else {
        return Ok(None);
    };
    let resource_path = node
        .parameters
        .get(parameter)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            connection_type_unavailable(format!(
                "node '{}' has no string resource binding in protocol parameter '{}'",
                node.id, parameter
            ))
        })?;
    let lookup = CatalogResourcePath::new(resource_path);
    let (canonical_path, resource) =
        resources.resources.get_key_value(&lookup).ok_or_else(|| {
            connection_type_unavailable(format!("bound resource '{resource_path}' is unavailable"))
        })?;
    if resource.display_kind() != *kind {
        return Err(connection_type_unavailable(format!(
            "bound resource '{}' does not match node protocol '{}'",
            canonical_path.as_str(),
            protocol.type_id
        )));
    }
    Ok(Some((canonical_path, resource)))
}

fn connection_type_unavailable(detail: impl Into<Box<str>>) -> EditorMutationError {
    mutation_validation_error(
        EditorMutationErrorCode::GraphConnectionTypeUnavailable,
        detail,
    )
}

pub fn filter_compatible_catalog(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    source: &PortAddress,
    catalog: &ResourceCatalogSnapshot,
    resources: &[CatalogResourceEntry],
    mut localized: LocalizedCatalog,
) -> Result<LocalizedCatalog, CatalogCompatibilityError> {
    let source = catalog_query_source_port(document, registry, source, catalog)?;
    localized.items.retain(|item| {
        let Ok(node_type) = yss_graph_protocol::NodeTypeId::new(item.node_type_id.as_ref()) else {
            return false;
        };
        let resource = item.resource_path.as_ref().and_then(|path| {
            resources.iter().find(|entry| {
                entry.resource_path.as_str() == path.as_str() && entry.node_type_id == node_type
            })
        });
        catalog_query_candidate_ports(graph_path, &node_type, resource, registry, catalog)
            .is_some_and(|candidates| {
                candidates
                    .iter()
                    .any(|candidate| ports_are_compatible(&source, candidate))
            })
    });
    let categories = localized
        .items
        .iter()
        .map(|item| item.category_id.as_ref())
        .collect::<BTreeSet<_>>();
    localized
        .categories
        .retain(|category| categories.contains(category.category_id.as_ref()));
    Ok(localized)
}

pub(crate) fn catalog_query_source_port(
    document: &GraphDocument,
    registry: &NodeRegistry,
    address: &PortAddress,
    catalog: &ResourceCatalogSnapshot,
) -> Result<SourcePort, CatalogCompatibilityError> {
    let resolved = resolve_editor_port(document, registry, address)
        .map_err(|_| CatalogCompatibilityError::SourceInvalid)?;
    if matches!(resolved.binding, Some(DynamicPortBinding::Orphan { .. })) {
        return Err(CatalogCompatibilityError::SourceInvalid);
    }
    let mut value_type = resolved.spec.value_type.clone();
    refine_catalog_query_resource_type(
        &mut value_type,
        &document.nodes[&address.node_id],
        resolved.protocol,
        catalog,
    )?;
    Ok(SourcePort {
        address: address.clone(),
        direction: resolved.spec.direction,
        value_type,
        type_parameters: resolved.protocol.interface.type_parameters.clone(),
    })
}

fn refine_catalog_query_resource_type(
    value_type: &mut TypeExpr,
    node: &DocumentNode,
    protocol: &NodeProtocol,
    catalog: &ResourceCatalogSnapshot,
) -> Result<(), CatalogCompatibilityError> {
    let NodeInstanceDisplaySpec::ResourceParameter { parameter, kind } = &protocol.instance_display
    else {
        return Ok(());
    };
    let resource = node
        .parameters
        .get(parameter)
        .and_then(serde_json::Value::as_str)
        .ok_or(CatalogCompatibilityError::SourceInvalid)?;
    let resource = GraphResourceId::new(resource);
    match kind {
        ResourceDisplayKind::Variable => {
            let variable = catalog
                .variable_contract(&resource)
                .ok_or(CatalogCompatibilityError::SourceInvalid)?;
            *value_type = yss_graph_type_mapping::type_expr_from_data_type(variable.data_type())
                .map_err(|_| CatalogCompatibilityError::SourceInvalid)?;
        }
        ResourceDisplayKind::Database => {
            catalog
                .database_schema(&resource)
                .ok_or(CatalogCompatibilityError::SourceInvalid)?;
            *value_type = yss_graph_type_mapping::type_expr_from_data_type(&DataType::DataFrame)
                .map_err(|_| CatalogCompatibilityError::SourceInvalid)?;
        }
        ResourceDisplayKind::Function => {}
    }
    Ok(())
}

fn catalog_query_candidate_ports(
    graph_path: &GraphResourcePath,
    node_type: &yss_graph_protocol::NodeTypeId,
    resource: Option<&CatalogResourceEntry>,
    registry: &NodeRegistry,
    catalog: &ResourceCatalogSnapshot,
) -> Option<Vec<CandidatePort>> {
    let protocol = registry.protocol(node_type)?;
    validate_scope(graph_path, protocol).ok()?;
    let mut candidates = protocol
        .interface
        .ports
        .iter()
        .filter(|port| match port.cardinality {
            PortCardinality::Declared => true,
            PortCardinality::UserCreated { min, .. } => min > 0,
            PortCardinality::Derived { .. } => false,
        })
        .map(|port| CandidatePort {
            template: port.key.clone(),
            direction: port.direction,
            connections: port.connections,
            value_type: port.value_type.clone(),
            type_parameters: protocol.interface.type_parameters.clone(),
            dynamic: None,
        })
        .collect::<Vec<_>>();

    let Some(resource) = resource else {
        return Some(candidates);
    };
    resource_parameter(protocol, resource.create_args).ok()?;
    if !resource_path_is_valid(&resource.resource_path, resource.create_args) {
        return None;
    }
    match resource.create_args {
        ResourceBoundCreateArgs::Variable => {
            let variable = catalog
                .variable_contract(&GraphResourceId::new(resource.resource_path.as_str()))?;
            let value_type =
                yss_graph_type_mapping::type_expr_from_data_type(variable.data_type()).ok()?;
            override_data_candidate_types(&mut candidates, value_type);
        }
        ResourceBoundCreateArgs::Database => {
            catalog.database_schema(&GraphResourceId::new(resource.resource_path.as_str()))?;
            let value_type =
                yss_graph_type_mapping::type_expr_from_data_type(&DataType::DataFrame).ok()?;
            override_data_candidate_types(&mut candidates, value_type);
        }
        ResourceBoundCreateArgs::Function => {
            let function_path = GraphResourcePath::new(resource.resource_path.as_str()).ok()?;
            let signature = catalog.function_signature(&function_path)?;
            let arguments = protocol
                .interface
                .ports
                .iter()
                .find(|port| port.key.as_str() == "arguments");
            if let Some(arguments) = arguments {
                for (index, parameter) in signature.parameters().iter().enumerate() {
                    candidates.push(CandidatePort {
                        template: arguments.key.clone(),
                        direction: arguments.direction,
                        connections: arguments.connections,
                        value_type: yss_graph_type_mapping::type_expr_from_data_type(
                            parameter.data_type(),
                        )
                        .ok()?,
                        type_parameters: Box::new([]),
                        dynamic: Some(DynamicCandidate {
                            origin: DynamicMemberLocator::FunctionParameter {
                                function: function_path.clone(),
                                parameter: parameter.id().clone(),
                            },
                            order: OrderKey::new(format!("{index:05}")),
                            last_known: LastKnownPortMetadata {
                                label: parameter.name().to_owned(),
                                value_type: Some(
                                    yss_graph_type_mapping::type_expr_from_data_type(
                                        parameter.data_type(),
                                    )
                                    .ok()?,
                                ),
                            },
                        }),
                    });
                }
            }
            if let (Some(results), Some(data_type)) = (
                protocol
                    .interface
                    .ports
                    .iter()
                    .find(|port| port.key.as_str() == "results"),
                signature.result(),
            ) {
                candidates.push(CandidatePort {
                    template: results.key.clone(),
                    direction: results.direction,
                    connections: results.connections,
                    value_type: yss_graph_type_mapping::type_expr_from_data_type(data_type).ok()?,
                    type_parameters: Box::new([]),
                    dynamic: Some(DynamicCandidate {
                        origin: DynamicMemberLocator::FunctionParameter {
                            function: function_path,
                            parameter: FunctionParameterId::new("return"),
                        },
                        order: OrderKey::new("00000"),
                        last_known: LastKnownPortMetadata {
                            label: "Result".to_owned(),
                            value_type: Some(
                                yss_graph_type_mapping::type_expr_from_data_type(data_type).ok()?,
                            ),
                        },
                    }),
                });
            }
        }
    }
    Some(candidates)
}

fn override_data_candidate_types(candidates: &mut [CandidatePort], value_type: TypeExpr) {
    for candidate in candidates {
        candidate.value_type = value_type.clone();
    }
}

pub(crate) fn connection_candidates(
    graph_path: &GraphResourcePath,
    descriptor: &NodeCreation,
    registry: &NodeRegistry,
    resources: &CatalogMutationValidationSnapshot,
    source: &SourcePort,
) -> Result<Vec<CandidatePort>, String> {
    let candidates = candidate_ports(graph_path, descriptor, registry, resources)?
        .into_iter()
        .filter(|candidate| ports_are_compatible(source, candidate))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        Err("created node has no compatible opposite-direction port".into())
    } else {
        Ok(candidates)
    }
}

fn candidate_ports(
    graph_path: &GraphResourcePath,
    descriptor: &NodeCreation,
    registry: &NodeRegistry,
    resources: &CatalogMutationValidationSnapshot,
) -> Result<Vec<CandidatePort>, String> {
    let node_type = descriptor_node_type(descriptor);
    let protocol = registry
        .protocol(node_type)
        .ok_or_else(|| format!("unknown node type '{node_type}'"))?;
    validate_scope(graph_path, protocol)?;
    let resource = descriptor_resource(descriptor, protocol, resources)?;
    if let Some(CatalogMutationResource::Variable { scope, .. }) = resource
        && !variable_in_scope(graph_path, scope)
    {
        return Err("variable resource is out of graph scope".into());
    }
    let mut ports = protocol
        .interface
        .ports
        .iter()
        .filter(|spec| match spec.cardinality {
            PortCardinality::Declared => true,
            PortCardinality::UserCreated { min, .. } => min > 0,
            PortCardinality::Derived { .. } => false,
        })
        .map(|spec| {
            Ok(CandidatePort {
                template: spec.key.clone(),
                direction: spec.direction,
                connections: spec.connections,
                value_type: resource_type_override(resource)?
                    .unwrap_or_else(|| spec.value_type.clone()),
                type_parameters: protocol.interface.type_parameters.clone(),
                dynamic: None,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if let Some(CatalogMutationResource::Function { signature, .. }) = resource {
        let function = match descriptor {
            NodeCreation::ResourceBound { resource_path, .. } => {
                GraphResourcePath::new(resource_path.as_str())
                    .map_err(|_| "function resource path is invalid".to_owned())?
            }
            _ => unreachable!(),
        };
        let argument_spec = protocol
            .interface
            .ports
            .iter()
            .find(|port| port.key.as_str() == "arguments");
        if let Some(spec) = argument_spec {
            for (index, parameter) in signature.parameters.iter().enumerate() {
                ports.push(CandidatePort {
                    template: spec.key.clone(),
                    direction: spec.direction,
                    connections: spec.connections,
                    value_type: function_type_expr(&parameter.type_name)?,
                    type_parameters: Box::new([]),
                    dynamic: Some(DynamicCandidate {
                        origin: DynamicMemberLocator::FunctionParameter {
                            function: function.clone(),
                            parameter: parameter.id.clone(),
                        },
                        order: OrderKey::new(format!("{index:05}")),
                        last_known: LastKnownPortMetadata {
                            label: parameter.name.clone(),
                            value_type: Some(function_type_expr(&parameter.type_name)?),
                        },
                    }),
                });
            }
        }
        if let (Some(spec), Some(return_type)) = (
            protocol
                .interface
                .ports
                .iter()
                .find(|port| port.key.as_str() == "results"),
            signature.return_type.as_deref(),
        ) {
            ports.push(CandidatePort {
                template: spec.key.clone(),
                direction: spec.direction,
                connections: spec.connections,
                value_type: function_type_expr(return_type)?,
                type_parameters: Box::new([]),
                dynamic: Some(DynamicCandidate {
                    origin: DynamicMemberLocator::FunctionParameter {
                        function,
                        parameter: FunctionParameterId::new("return"),
                    },
                    order: OrderKey::new("00000"),
                    last_known: LastKnownPortMetadata {
                        label: return_type.to_owned(),
                        value_type: Some(function_type_expr(return_type)?),
                    },
                }),
            });
        }
    }
    Ok(ports)
}

fn descriptor_node_type(descriptor: &NodeCreation) -> &yss_graph_protocol::NodeTypeId {
    match descriptor {
        NodeCreation::Static { node_type_id }
        | NodeCreation::ParameterizedStatic { node_type_id, .. }
        | NodeCreation::ResourceBound { node_type_id, .. } => node_type_id,
    }
}

fn descriptor_resource<'a>(
    descriptor: &NodeCreation,
    protocol: &NodeProtocol,
    resources: &'a CatalogMutationValidationSnapshot,
) -> Result<Option<&'a CatalogMutationResource>, String> {
    let NodeCreation::ResourceBound {
        resource_path,
        resource_revision,
        create_args,
        ..
    } = descriptor
    else {
        return Ok(None);
    };
    let resource = resources.resources.get(resource_path).ok_or_else(|| {
        format!(
            "catalog resource '{}' is unavailable",
            resource_path.as_str()
        )
    })?;
    resource_parameter(protocol, *create_args)?;
    let valid = resource.create_args() == *create_args && resource.revision() == *resource_revision;
    valid
        .then_some(Some(resource))
        .ok_or_else(|| "catalog resource descriptor is stale or invalid".into())
}

fn resource_type_override(
    resource: Option<&CatalogMutationResource>,
) -> Result<Option<TypeExpr>, String> {
    match resource {
        Some(CatalogMutationResource::Variable { data_type, .. }) => {
            editor_type_expr(data_type).map(Some)
        }
        Some(CatalogMutationResource::Database { .. }) => {
            editor_type_expr(&DataType::DataFrame).map(Some)
        }
        _ => Ok(None),
    }
}

pub(crate) fn function_type_expr(type_name: &str) -> Result<TypeExpr, String> {
    yss_graph_type_mapping::type_expr_from_data_type_name(type_name)
        .map_err(|error| error.to_string())
}

fn editor_type_expr(data_type: &DataType) -> Result<TypeExpr, String> {
    yss_graph_type_mapping::type_expr_from_data_type(data_type).map_err(|error| error.to_string())
}

fn validate_scope(graph_path: &GraphResourcePath, protocol: &NodeProtocol) -> Result<(), String> {
    let allowed = match protocol.scope {
        yss_graph_protocol::NodeScope::Any => true,
        yss_graph_protocol::NodeScope::Function => graph_path.kind() == GraphResourceKind::Function,
    };
    if !allowed {
        Err(format!(
            "node type '{}' is out of graph scope",
            protocol.type_id
        ))
    } else {
        Ok(())
    }
}

fn ports_are_compatible(source: &SourcePort, candidate: &CandidatePort) -> bool {
    if source.direction == candidate.direction {
        return false;
    }
    let compatibility = match source.direction {
        PortDirection::Output => yss_graph_protocol::type_exprs_compatibility(
            &source.value_type,
            &candidate.value_type,
            &source.type_parameters,
            &candidate.type_parameters,
        ),
        PortDirection::Input => yss_graph_protocol::type_exprs_compatibility(
            &candidate.value_type,
            &source.value_type,
            &candidate.type_parameters,
            &source.type_parameters,
        ),
    };
    compatibility != yss_graph_protocol::TypeCompatibility::Incompatible
        || match source.direction {
            PortDirection::Output => !type_pattern_is_exact(&source.value_type),
            PortDirection::Input => !type_pattern_is_exact(&candidate.value_type),
        }
}
