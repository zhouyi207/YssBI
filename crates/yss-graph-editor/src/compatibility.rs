use crate::mutation::{EditorMutationError, EditorMutationErrorCode};
use std::collections::{BTreeMap, BTreeSet};
use yss_data_contract::ValueType;
use yss_graph_analysis::GraphSemanticSnapshot;
use yss_graph_document::{
    DocumentNode, DynamicMemberLocator, DynamicPortBinding, FunctionParameterId, GraphDocument,
    GraphResourceKind, GraphResourcePath, LastKnownPortMetadata, NodeId, OrderKey, PortAddress,
    PortRef,
};
use yss_graph_resource_contract::{FunctionSignature, GraphResourceId, ResourceCatalogSnapshot};
use yss_node_catalog::{
    CatalogResourceEntry, CatalogResourcePath, LocalizedCatalog, ResourceBoundCreateArgs,
};
use yss_node_protocol::{
    ConnectionsPerPort, NodeInstanceDisplaySpec, NodeProtocol, ParameterKey, PortCardinality,
    PortDirection, PortKey, PortSpec, ResourceDisplayKind, TypeExpr,
};
use yss_node_registry::NodeRegistry;

#[derive(Clone, Debug, PartialEq)]
/// Editor-facing catalog authority used to validate creation descriptors and dynamic ports.
///
/// This is intentionally richer than `yss_graph_resource_contract::ResourceCatalogSnapshot`,
/// which owns only the type/schema facts required by graph analysis. Project/session
/// currentness remains enforced by the caller's graph-operation commit authority.
pub struct CatalogMutationValidationSnapshot {
    pub resources: BTreeMap<CatalogResourcePath, CatalogMutationResource>,
}

#[derive(Clone, Copy, Default)]
/// Borrowed authorities for the document before this mutation. Ports introduced
/// by the same atomic patch are validated from their registry/catalog declarations.
pub struct EditorMutationContext<'a> {
    pub catalog: Option<&'a CatalogMutationValidationSnapshot>,
    pub semantics: Option<&'a GraphSemanticSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CatalogMutationResource {
    Function {
        revision: u64,
        signature: FunctionSignature,
    },
    Database {
        authority_revision: u64,
    },
}

impl CatalogMutationResource {
    pub(crate) fn create_args(&self) -> ResourceBoundCreateArgs {
        match self {
            Self::Function { .. } => ResourceBoundCreateArgs::FunctionGraph,
            Self::Database { .. } => ResourceBoundCreateArgs::Database,
        }
    }

    pub(crate) fn display_kind(&self) -> ResourceDisplayKind {
        resource_display_kind(self.create_args())
    }

    pub(crate) fn revision(&self) -> u64 {
        match self {
            Self::Function { revision, .. } => *revision,
            Self::Database {
                authority_revision, ..
            } => *authority_revision,
        }
    }
}

fn resource_display_kind(create_args: ResourceBoundCreateArgs) -> ResourceDisplayKind {
    match create_args {
        ResourceBoundCreateArgs::FunctionGraph => ResourceDisplayKind::Function,
        ResourceBoundCreateArgs::Database => ResourceDisplayKind::Database,
    }
}

pub(crate) fn resource_path_is_valid(
    resource_path: &CatalogResourcePath,
    create_args: ResourceBoundCreateArgs,
) -> bool {
    let path = resource_path.as_str();
    match create_args {
        ResourceBoundCreateArgs::FunctionGraph => {
            GraphResourcePath::new(path).is_ok_and(|canonical| {
                canonical.as_str() == path && canonical.as_str().starts_with("functions/")
            })
        }
        ResourceBoundCreateArgs::Database => path
            .strip_prefix("databases/")
            .is_some_and(|id| !id.is_empty()),
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
pub struct SourcePort {
    pub address: PortAddress,
    pub direction: PortDirection,
    pub value_type: TypeExpr,
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
    context: EditorMutationContext<'_>,
    address: PortAddress,
) -> Result<SourcePort, EditorMutationError> {
    let resolved = resolve_editor_port(document, registry, &address)?;
    if let Some(port) = context
        .semantics
        .and_then(|snapshot| snapshot.concrete_interface().port(&address))
    {
        if port.orphan {
            return Err(mutation_validation_error(
                EditorMutationErrorCode::GraphPortOrphan,
                "orphan ports cannot be connected",
            ));
        }
        return Ok(SourcePort {
            address,
            direction: port.direction,
            value_type: port.connection_type(),
        });
    }
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
    };
    refine_constant_type(&mut source.value_type, &source.address, document, protocol);
    if let Some(resources) = context.catalog {
        refine_source_type(&mut source, document, protocol, spec, resources)?;
    }
    Ok(source)
}

pub(crate) fn validate_connection_types(
    document: &GraphDocument,
    registry: &NodeRegistry,
    context: EditorMutationContext<'_>,
    output: &PortAddress,
    input: &PortAddress,
) -> Result<(), EditorMutationError> {
    if output.node_id == input.node_id {
        return Err(mutation_validation_error(
            EditorMutationErrorCode::GraphConnectionSameNode,
            "connection endpoints must belong to different nodes",
        ));
    }
    let output = source_port(document, registry, context, output.clone())?;
    let input = source_port(document, registry, context, input.clone())?;
    if output.direction != PortDirection::Output || input.direction != PortDirection::Input {
        return Err(mutation_validation_error(
            EditorMutationErrorCode::GraphConnectionDirectionMismatch,
            "connection endpoints have invalid directions",
        ));
    }
    if yss_graph_analysis::type_patterns_can_connect(
        &output.value_type,
        &input.value_type,
        registry.types(),
    ) {
        Ok(())
    } else {
        Err(mutation_validation_error(
            EditorMutationErrorCode::GraphConnectionTypeMismatch,
            "connection endpoint types are not assignable",
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidatePort {
    pub template: PortKey,
    pub direction: PortDirection,
    pub connections: ConnectionsPerPort,
    pub value_type: TypeExpr,
    pub dynamic: Option<DynamicCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicCandidate {
    pub origin: DynamicMemberLocator,
    pub order: OrderKey,
    pub last_known: LastKnownPortMetadata,
}

fn refine_source_type(
    source: &mut SourcePort,
    document: &GraphDocument,
    protocol: &NodeProtocol,
    spec: &PortSpec,
    resources: &CatalogMutationValidationSnapshot,
) -> Result<(), EditorMutationError> {
    let Some(node) = document.nodes.get(&source.address.node_id) else {
        return Ok(());
    };
    let Some((resource_path, resource)) = bound_catalog_resource(node, protocol, resources)? else {
        return Ok(());
    };
    match resource {
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
            let data_type =
                function_member_data_type(signature, spec, parameter).ok_or_else(|| {
                    connection_type_unavailable(format!(
                        "function resource '{}' has no authoritative type for member '{}'",
                        resource_path.as_str(),
                        parameter.as_str()
                    ))
                })?;
            source.value_type = editor_type_expr(data_type).map_err(|error| {
                connection_type_unavailable(format!(
                    "function resource '{}' member '{}' has an invalid authoritative type: {error}",
                    resource_path.as_str(),
                    parameter.as_str()
                ))
            })?;
        }
        CatalogMutationResource::Database { .. } => {
            source.value_type = editor_type_expr(&ValueType::DataFrame).map_err(|error| {
                connection_type_unavailable(format!(
                    "database resource '{}' has an invalid authoritative type: {error}",
                    resource_path.as_str()
                ))
            })?;
        }
    }
    Ok(())
}

pub(crate) fn function_member_data_type<'a>(
    signature: &'a FunctionSignature,
    spec: &PortSpec,
    parameter: &FunctionParameterId,
) -> Option<&'a ValueType> {
    let PortCardinality::Derived { resolver } = &spec.cardinality else {
        return None;
    };
    match (resolver.as_str(), spec.direction) {
        (yss_node_registry::FUNCTION_CALL_ARGUMENTS_RESOLVER, PortDirection::Input) => signature
            .parameters()
            .iter()
            .find(|candidate| candidate.id() == parameter)
            .map(|candidate| candidate.data_type()),
        (yss_node_registry::FUNCTION_CALL_RESULTS_RESOLVER, PortDirection::Output)
            if parameter.as_str() == "return" =>
        {
            signature.result()
        }
        _ => None,
    }
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
    let resource_path = protocol
        .parameters
        .effective_text(parameter, &node.parameters)
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

/// Query sources are resolved and validated against the current semantic snapshot by Graph Runtime.
pub fn filter_compatible_catalog(
    graph_path: &GraphResourcePath,
    registry: &NodeRegistry,
    source: &SourcePort,
    catalog: &ResourceCatalogSnapshot,
    resources: &[CatalogResourceEntry],
    mut localized: LocalizedCatalog,
) -> LocalizedCatalog {
    localized.items.retain(|item| {
        let Ok(node_type) = yss_node_protocol::NodeTypeId::new(item.node_type_id.as_ref()) else {
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
                    .any(|candidate| ports_are_compatible(source, candidate, registry))
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
    localized
}

fn refine_constant_type(
    value_type: &mut TypeExpr,
    address: &PortAddress,
    document: &GraphDocument,
    protocol: &NodeProtocol,
) {
    let yss_node_protocol::NodeTypingSpec::ConstantOutput { output, .. } = &protocol.typing else {
        return;
    };
    if address != &PortAddress::declared(address.node_id, output.clone()) {
        return;
    }
    let constant = document
        .nodes
        .get(&address.node_id)
        .and_then(|node| yss_graph_analysis::referenced_constant(document, node, protocol));
    if let Some(resolved) = constant.and_then(|constant| {
        yss_graph_type_mapping::type_expr_from_data_type(&constant.data_type).ok()
    }) {
        *value_type = resolved;
    }
}

fn catalog_query_candidate_ports(
    graph_path: &GraphResourcePath,
    node_type: &yss_node_protocol::NodeTypeId,
    resource: Option<&CatalogResourceEntry>,
    registry: &NodeRegistry,
    catalog: &ResourceCatalogSnapshot,
) -> Option<Vec<CandidatePort>> {
    let protocol = registry.protocol(node_type)?;
    validate_scope(graph_path, protocol).ok()?;
    let mut candidates = initial_candidate_ports(protocol, |port| {
        protocol.interface.port_instance_bounds(port).0 > 0
    });

    let Some(resource) = resource else {
        return Some(candidates);
    };
    resource_parameter(protocol, resource.create_args).ok()?;
    if !resource_path_is_valid(&resource.resource_path, resource.create_args) {
        return None;
    }
    match resource.create_args {
        ResourceBoundCreateArgs::Database => {
            catalog.database_schema(&GraphResourceId::new(resource.resource_path.as_str()))?;
            let value_type = editor_type_expr(&ValueType::DataFrame).ok()?;
            override_data_candidate_types(&mut candidates, value_type);
        }
        ResourceBoundCreateArgs::FunctionGraph => {
            let function_path = GraphResourcePath::new(resource.resource_path.as_str()).ok()?;
            let signature = catalog.function_signature(&function_path)?;
            append_function_candidate_ports(&mut candidates, protocol, &function_path, signature)
                .ok()?;
        }
    }
    Some(candidates)
}

fn initial_candidate_ports(
    protocol: &NodeProtocol,
    has_instances: impl Fn(&PortSpec) -> bool,
) -> Vec<CandidatePort> {
    protocol
        .interface
        .ports
        .iter()
        .filter(|port| match port.cardinality {
            PortCardinality::Declared => true,
            PortCardinality::UserCreated { .. } => has_instances(port),
            PortCardinality::Derived { .. } => false,
        })
        .map(|port| CandidatePort {
            template: port.key.clone(),
            direction: port.direction,
            connections: port.connections,
            value_type: port.value_type.clone(),
            dynamic: None,
        })
        .collect()
}

fn append_function_candidate_ports(
    candidates: &mut Vec<CandidatePort>,
    protocol: &NodeProtocol,
    function: &GraphResourcePath,
    signature: &FunctionSignature,
) -> Result<(), String> {
    for spec in &protocol.interface.ports {
        let PortCardinality::Derived { resolver } = &spec.cardinality else {
            continue;
        };
        match (resolver.as_str(), spec.direction) {
            (yss_node_registry::FUNCTION_CALL_ARGUMENTS_RESOLVER, PortDirection::Input) => {
                for (index, parameter) in signature.parameters().iter().enumerate() {
                    candidates.push(function_candidate(
                        spec,
                        function,
                        parameter.id().clone(),
                        parameter.name(),
                        index,
                        parameter.data_type(),
                    )?);
                }
            }
            (yss_node_registry::FUNCTION_CALL_RESULTS_RESOLVER, PortDirection::Output) => {
                if let Some(data_type) = signature.result() {
                    candidates.push(function_candidate(
                        spec,
                        function,
                        FunctionParameterId::new("return"),
                        "Result",
                        0,
                        data_type,
                    )?);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn function_candidate(
    spec: &PortSpec,
    function: &GraphResourcePath,
    parameter: FunctionParameterId,
    label: &str,
    index: usize,
    data_type: &ValueType,
) -> Result<CandidatePort, String> {
    let value_type = editor_type_expr(data_type)?;
    Ok(CandidatePort {
        template: spec.key.clone(),
        direction: spec.direction,
        connections: spec.connections,
        value_type: value_type.clone(),
        dynamic: Some(DynamicCandidate {
            origin: DynamicMemberLocator::FunctionParameter {
                function: function.clone(),
                parameter,
            },
            order: OrderKey::new(format!("{index:05}")),
            last_known: LastKnownPortMetadata {
                label: label.to_owned(),
                value_type: Some(value_type),
            },
        }),
    })
}

fn override_data_candidate_types(candidates: &mut [CandidatePort], value_type: TypeExpr) {
    for candidate in candidates {
        candidate.value_type = value_type.clone();
    }
}

pub(crate) fn connection_candidate(
    document: &GraphDocument,
    node_id: NodeId,
    registry: &NodeRegistry,
    resources: &CatalogMutationValidationSnapshot,
    source: &SourcePort,
) -> Result<CandidatePort, EditorMutationError> {
    candidate_ports(document, node_id, registry, resources)?
        .into_iter()
        .find(|candidate| ports_are_compatible(source, candidate, registry))
        .ok_or_else(|| {
            mutation_validation_error(
                EditorMutationErrorCode::GraphConnectionTypeMismatch,
                "created node has no compatible opposite-direction port",
            )
        })
}

fn candidate_ports(
    document: &GraphDocument,
    node_id: NodeId,
    registry: &NodeRegistry,
    resources: &CatalogMutationValidationSnapshot,
) -> Result<Vec<CandidatePort>, EditorMutationError> {
    let node = document
        .nodes
        .get(&node_id)
        .ok_or_else(|| connection_type_unavailable("created node is unavailable"))?;
    let protocol = registry.protocol(&node.node_type).ok_or_else(|| {
        connection_type_unavailable(format!("unknown node type '{}'", node.node_type))
    })?;
    let mut ports = initial_candidate_ports(protocol, |port| {
        yss_graph_document_edit::user_created_port_instance_count(
            node_id,
            &port.key,
            document.port_bindings.iter(),
        ) > 0
    });
    for port in &mut ports {
        refine_constant_type(
            &mut port.value_type,
            &PortAddress::declared(node_id, port.template.clone()),
            document,
            protocol,
        );
    }
    match bound_catalog_resource(node, protocol, resources)? {
        Some((_, CatalogMutationResource::Database { .. })) => {
            override_data_candidate_types(
                &mut ports,
                editor_type_expr(&ValueType::DataFrame).map_err(connection_type_unavailable)?,
            );
        }
        Some((resource_path, CatalogMutationResource::Function { signature, .. })) => {
            let function = GraphResourcePath::new(resource_path.as_str())
                .map_err(|_| connection_type_unavailable("function resource path is invalid"))?;
            append_function_candidate_ports(&mut ports, protocol, &function, signature)
                .map_err(connection_type_unavailable)?;
        }
        None => {}
    }
    Ok(ports)
}

pub(crate) fn editor_type_expr(data_type: &ValueType) -> Result<TypeExpr, String> {
    yss_graph_type_mapping::type_expr_from_data_type(data_type).map_err(|error| error.to_string())
}

fn validate_scope(graph_path: &GraphResourcePath, protocol: &NodeProtocol) -> Result<(), String> {
    let allowed = match protocol.scope {
        yss_node_protocol::NodeScope::Any => true,
        yss_node_protocol::NodeScope::Function => {
            graph_path.kind() == GraphResourceKind::FunctionGraph
        }
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

fn ports_are_compatible(
    source: &SourcePort,
    candidate: &CandidatePort,
    registry: &NodeRegistry,
) -> bool {
    if source.direction == candidate.direction {
        return false;
    }
    let (output, input) = match source.direction {
        PortDirection::Output => (&source.value_type, &candidate.value_type),
        PortDirection::Input => (&candidate.value_type, &source.value_type),
    };
    yss_graph_analysis::type_patterns_can_connect(output, input, registry.types())
}
