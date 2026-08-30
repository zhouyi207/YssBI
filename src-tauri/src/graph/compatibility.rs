use crate::graph::catalog::CatalogResourcePath;
use crate::graph::catalog::{NodeCreation, ResourceBoundCreateArgs};
use crate::graph::document::{EditorMutationError, EditorMutationErrorCode};
use std::collections::{BTreeMap, BTreeSet};
use yss_data_contract::DataType;
use yss_graph_document::{
    DynamicMemberLocator, FunctionParameterId, GraphDocument, GraphResourcePath, GraphRevision,
    LastKnownPortMetadata, OrderKey, PortAddress, PortRef,
};
use yss_graph_protocol::{
    ConnectionsPerPort, NodeProtocol, NodeTypeId, PortDirection, PortInstances, PortKey, PortKind,
    TypeConstructorId, TypeExpr, TypeId, TypeParameterId,
};
use yss_graph_registry::NodeRegistry;
use yss_variable_contract::VariableScope;

#[derive(Clone, Debug, PartialEq)]
pub struct CatalogMutationValidationSnapshot {
    pub authority_generation: u64,
    pub resources: BTreeMap<CatalogResourcePath, CatalogMutationResource>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CatalogFunctionParameter {
    pub(crate) id: yss_graph_document::FunctionParameterId,
    pub(crate) name: String,
    pub(crate) type_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CatalogFunctionSignature {
    pub(crate) parameters: Box<[CatalogFunctionParameter]>,
    pub(crate) return_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CatalogMutationResource {
    Function {
        revision: u64,
        signature: CatalogFunctionSignature,
        allowed_node_type_id: NodeTypeId,
        parameter_binding: Box<str>,
    },
    Variable {
        revision: u64,
        scope: VariableScope,
        data_type: DataType,
        allowed_node_type_ids: [NodeTypeId; 2],
        parameter_binding: Box<str>,
    },
    Database {
        authority_revision: u64,
        allowed_node_type_id: NodeTypeId,
        parameter_binding: Box<str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorMutationValidationSnapshot {
    pub graph_revision: GraphRevision,
    pub(crate) ports: BTreeMap<PortAddress, EditorMutationPortValidation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorMutationPortValidation {
    pub direction: PortDirection,
    pub kind: PortKind,
    pub orphan: bool,
    pub port_type: EditorMutationPortType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EditorMutationPortType {
    NotApplicable,
    Ready {
        expression: TypeExpr,
        type_parameters: Box<[TypeParameterId]>,
    },
    MissingResolvedType,
    MissingInternalTypeExpr,
    Unresolved {
        expression: TypeExpr,
        type_parameters: Box<[TypeParameterId]>,
    },
}

impl EditorMutationValidationSnapshot {
    pub(crate) fn validate_connection_endpoints(
        &self,
        output: &PortAddress,
        input: &PortAddress,
    ) -> Result<(), EditorMutationError> {
        let (output_port, input_port) = self.connection_ports(output, input)?;
        validate_port_endpoints(output_port, input_port)
    }

    pub(crate) fn validate_connection_types(
        &self,
        output: &PortAddress,
        input: &PortAddress,
    ) -> Result<(), EditorMutationError> {
        let (output_port, input_port) = self.connection_ports(output, input)?;
        validate_port_endpoints(output_port, input_port)?;
        validate_port_types(output_port, input_port)
    }

    pub(crate) fn validate_create_connection(
        &self,
        source: &PortAddress,
        candidate: &CandidatePort,
    ) -> Result<(), EditorMutationError> {
        let source_port = self.ports.get(source).ok_or_else(|| {
            mutation_validation_error(
                EditorMutationErrorCode::GraphPortNotFound,
                format!(
                    "create-and-connect source port '{source}' is absent from validation snapshot"
                ),
            )
        })?;
        let candidate_port = EditorMutationPortValidation {
            direction: candidate.direction,
            kind: candidate.kind,
            orphan: false,
            port_type: candidate_validation_type(candidate),
        };
        let (output_port, input_port) = match source_port.direction {
            PortDirection::Output => (source_port, &candidate_port),
            PortDirection::Input => (&candidate_port, source_port),
        };
        validate_port_endpoints(output_port, input_port)?;
        validate_port_types(output_port, input_port)
    }

    fn connection_ports(
        &self,
        output: &PortAddress,
        input: &PortAddress,
    ) -> Result<(&EditorMutationPortValidation, &EditorMutationPortValidation), EditorMutationError>
    {
        let output_port = self.ports.get(output).ok_or_else(|| {
            mutation_validation_error(
                EditorMutationErrorCode::GraphPortNotFound,
                format!("output port '{output}' is absent from validation snapshot"),
            )
        })?;
        let input_port = self.ports.get(input).ok_or_else(|| {
            mutation_validation_error(
                EditorMutationErrorCode::GraphPortNotFound,
                format!("input port '{input}' is absent from validation snapshot"),
            )
        })?;
        Ok((output_port, input_port))
    }
}

fn validate_port_endpoints(
    output: &EditorMutationPortValidation,
    input: &EditorMutationPortValidation,
) -> Result<(), EditorMutationError> {
    if output.orphan || input.orphan {
        return Err(mutation_validation_error(
            EditorMutationErrorCode::GraphPortOrphan,
            "orphan ports cannot be connected",
        ));
    }
    if output.direction != PortDirection::Output || input.direction != PortDirection::Input {
        return Err(mutation_validation_error(
            EditorMutationErrorCode::GraphConnectionDirectionMismatch,
            "connection endpoints have invalid directions",
        ));
    }
    if output.kind != input.kind {
        return Err(mutation_validation_error(
            EditorMutationErrorCode::GraphConnectionKindMismatch,
            "connection endpoint kinds do not match",
        ));
    }
    Ok(())
}

fn validate_port_types(
    output: &EditorMutationPortValidation,
    input: &EditorMutationPortValidation,
) -> Result<(), EditorMutationError> {
    match (&output.port_type, &input.port_type) {
        (EditorMutationPortType::NotApplicable, EditorMutationPortType::NotApplicable) => Ok(()),
        (
            EditorMutationPortType::MissingResolvedType
            | EditorMutationPortType::MissingInternalTypeExpr,
            _,
        )
        | (
            _,
            EditorMutationPortType::MissingResolvedType
            | EditorMutationPortType::MissingInternalTypeExpr,
        ) => Err(mutation_validation_error(
            EditorMutationErrorCode::GraphConnectionTypeUnavailable,
            "connection endpoint projection has no authoritative type expression",
        )),
        (EditorMutationPortType::Unresolved { .. }, _)
        | (_, EditorMutationPortType::Unresolved { .. }) => Err(mutation_validation_error(
            EditorMutationErrorCode::GraphConnectionTypeUnresolved,
            "connection endpoint type expression is unresolved",
        )),
        (
            EditorMutationPortType::Ready {
                expression: source,
                type_parameters: source_type_parameters,
            },
            EditorMutationPortType::Ready {
                expression: target,
                type_parameters: target_type_parameters,
            },
        ) if yss_graph_protocol::type_exprs_compatibility(
            source,
            target,
            source_type_parameters,
            target_type_parameters,
        ) == yss_graph_protocol::TypeCompatibility::Compatible =>
        {
            Ok(())
        }
        (EditorMutationPortType::Ready { .. }, EditorMutationPortType::Ready { .. }) => {
            Err(mutation_validation_error(
                EditorMutationErrorCode::GraphConnectionTypeMismatch,
                "connection endpoint types are not assignable",
            ))
        }
        _ => Err(mutation_validation_error(
            EditorMutationErrorCode::GraphConnectionKindMismatch,
            "connection endpoint kinds do not match",
        )),
    }
}

fn candidate_validation_type(candidate: &CandidatePort) -> EditorMutationPortType {
    if candidate.kind != PortKind::Data {
        return EditorMutationPortType::NotApplicable;
    }
    if type_expr_is_unresolved(&candidate.value_type, &candidate.type_parameters) {
        EditorMutationPortType::Unresolved {
            expression: candidate.value_type.clone(),
            type_parameters: candidate.type_parameters.clone(),
        }
    } else {
        EditorMutationPortType::Ready {
            expression: candidate.value_type.clone(),
            type_parameters: candidate.type_parameters.clone(),
        }
    }
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

fn type_expr_is_unresolved(expression: &TypeExpr, parameters: &[TypeParameterId]) -> bool {
    let declared = parameters.iter().collect::<BTreeSet<_>>();
    fn visit(expression: &TypeExpr, declared: &BTreeSet<&TypeParameterId>) -> bool {
        match expression {
            TypeExpr::Concrete(_) => false,
            TypeExpr::Generic(id) => !declared.contains(id),
            TypeExpr::Applied { arguments, .. } | TypeExpr::Union(arguments) => {
                arguments.iter().any(|argument| visit(argument, declared))
            }
            TypeExpr::Unknown => true,
        }
    }
    visit(expression, &declared)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourcePort {
    pub address: PortAddress,
    pub direction: PortDirection,
    pub kind: PortKind,
    pub value_type: TypeExpr,
    pub type_parameters: Box<[TypeParameterId]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidatePort {
    pub template: PortKey,
    pub direction: PortDirection,
    pub kind: PortKind,
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

#[cfg(test)]
pub(crate) fn refine_source_type(
    source: &mut SourcePort,
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &CatalogMutationValidationSnapshot,
) {
    if source.kind != PortKind::Data
        || !matches!(source.value_type, TypeExpr::Generic(_) | TypeExpr::Unknown)
    {
        return;
    }
    let Some(node) = document.nodes.get(&source.address.node_id) else {
        return;
    };
    let Some(protocol) = registry.protocol(&node.node_type) else {
        return;
    };
    let template = match &source.address.port {
        PortRef::Declared { key } => key,
        PortRef::Instance { template, .. } => template,
    };
    let Some(spec) = protocol
        .interface
        .ports
        .iter()
        .find(|port| &port.key == template)
    else {
        return;
    };
    source.value_type = spec.value_type.clone();
    let Some(path) = node
        .parameters
        .values()
        .find_map(|value| value.as_str())
        .map(crate::graph::catalog::CatalogResourcePath::new)
    else {
        return;
    };
    let Some(resource) = resources.resources.get(&path) else {
        return;
    };
    match resource {
        CatalogMutationResource::Variable { data_type, .. } => {
            if let Ok(value_type) = data_type_to_type_expr(&data_type) {
                source.value_type = value_type;
            }
        }
        CatalogMutationResource::Function { signature, .. } => {
            let Some(binding) = document.port_bindings.get(&source.address) else {
                return;
            };
            let origin = match binding {
                yss_graph_document::DynamicPortBinding::Resolved { origin, .. }
                | yss_graph_document::DynamicPortBinding::Orphan { origin, .. } => origin,
                yss_graph_document::DynamicPortBinding::UserCreated { .. } => return,
            };
            let DynamicMemberLocator::FunctionParameter { parameter, .. } = origin else {
                return;
            };
            let type_name = if parameter.as_str() == "return" {
                signature.return_type.as_deref()
            } else {
                signature
                    .parameters
                    .iter()
                    .find(|candidate| candidate.id == *parameter)
                    .map(|candidate| candidate.type_name.as_str())
            };
            if let Some(Ok(value_type)) = type_name.map(function_type_expr) {
                source.value_type = value_type;
            }
        }
        CatalogMutationResource::Database { .. } => {}
    }
}

#[cfg(test)]
pub(crate) fn compatible_catalog(
    snapshot: &crate::project::CatalogProjectSnapshot,
    graph_path: &GraphResourcePath,
    source: &SourcePort,
    locale: &str,
) -> crate::schema::catalog::LocalizedCatalogDto {
    let mut localized = snapshot.catalog.localize_with_resources(
        snapshot.registry.as_ref(),
        locale,
        &snapshot.resources,
    );
    localized.items.retain(|item| {
        candidate_ports(
            graph_path,
            &item.creation,
            snapshot.registry.as_ref(),
            &snapshot.validation,
        )
        .is_ok_and(|ports| ports.iter().any(|port| ports_are_compatible(source, port)))
    });
    let categories = localized
        .items
        .iter()
        .map(|item| item.category_id.as_ref())
        .collect::<std::collections::BTreeSet<_>>();
    localized
        .categories
        .retain(|category| categories.contains(category.category_id.as_ref()));

    let mut dto = crate::schema::catalog::LocalizedCatalogDto::from(localized);
    dto.project_instance_id = snapshot.project_instance_id.as_str().into();
    dto.registry_fingerprint = snapshot.registry.fingerprint().to_string().into();
    dto.resource_publication_revision = snapshot.resource_publication_revision;
    dto
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
        .filter(|candidate| {
            source.direction != candidate.direction && source.kind == candidate.kind
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        Err("created node has no opposite-direction port with the same kind".into())
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
    let resource = descriptor_resource(descriptor, resources)?;
    if let Some(CatalogMutationResource::Variable { scope, .. }) = resource {
        let in_scope = match scope {
            yss_variable_contract::VariableScope::Global => true,
            yss_variable_contract::VariableScope::Event { event_path }
            | yss_variable_contract::VariableScope::Function {
                function_path: event_path,
            } => event_path.as_str() == graph_path.as_str(),
        };
        if !in_scope {
            return Err("variable resource is out of graph scope".into());
        }
    }
    let mut ports = protocol
        .interface
        .ports
        .iter()
        .filter(|spec| match spec.instances {
            PortInstances::Declared => true,
            PortInstances::UserCreated { min, .. } => min > 0,
            PortInstances::Derived { .. } => false,
        })
        .map(|spec| {
            Ok(CandidatePort {
                template: spec.key.clone(),
                direction: spec.direction,
                kind: spec.kind,
                connections: spec.connections,
                value_type: resource_type_override(resource, node_type.as_str())?
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
                    kind: spec.kind,
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
                kind: spec.kind,
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
    resources: &'a CatalogMutationValidationSnapshot,
) -> Result<Option<&'a CatalogMutationResource>, String> {
    let NodeCreation::ResourceBound {
        node_type_id,
        resource_path,
        resource_revision,
        create_args,
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
    let valid = match (create_args, resource) {
        (
            ResourceBoundCreateArgs::Function,
            CatalogMutationResource::Function {
                revision,
                allowed_node_type_id,
                ..
            },
        ) => *revision == *resource_revision && allowed_node_type_id == node_type_id,
        (
            ResourceBoundCreateArgs::Variable,
            CatalogMutationResource::Variable {
                revision,
                allowed_node_type_ids,
                ..
            },
        ) => {
            *revision == *resource_revision
                && allowed_node_type_ids
                    .iter()
                    .any(|allowed| allowed == node_type_id)
        }
        (
            ResourceBoundCreateArgs::Database,
            CatalogMutationResource::Database {
                authority_revision,
                allowed_node_type_id,
                ..
            },
        ) => *authority_revision == *resource_revision && allowed_node_type_id == node_type_id,
        _ => false,
    };
    valid
        .then_some(Some(resource))
        .ok_or_else(|| "catalog resource descriptor is stale or invalid".into())
}

fn resource_type_override(
    resource: Option<&CatalogMutationResource>,
    node_type: &str,
) -> Result<Option<TypeExpr>, String> {
    match resource {
        Some(CatalogMutationResource::Variable { data_type, .. }) => {
            data_type_to_type_expr(data_type).map(Some)
        }
        Some(CatalogMutationResource::Database { .. })
            if node_type == "yssbi.dataframe.source.get" =>
        {
            Ok(Some(concrete_type("tabular.dataframe")?))
        }
        _ => Ok(None),
    }
}

pub(crate) fn function_type_expr(type_name: &str) -> Result<TypeExpr, String> {
    let data_type = type_name
        .parse::<DataType>()
        .map_err(|error| error.to_string())?;
    data_type_to_type_expr(&data_type)
}

pub(crate) fn data_type_to_type_expr(data_type: &DataType) -> Result<TypeExpr, String> {
    match data_type {
        DataType::Boolean => concrete_type("core.bool"),
        DataType::Int64 => concrete_type("core.int64"),
        DataType::Float64 => concrete_type("core.float64"),
        DataType::String => concrete_type("core.string"),
        DataType::Date => concrete_type("core.date"),
        DataType::Datetime => concrete_type("core.datetime"),
        DataType::Time => concrete_type("core.time"),
        DataType::Categorical => concrete_type("core.categorical"),
        DataType::Object => concrete_type("core.object"),
        DataType::DataFrame => concrete_type("tabular.dataframe"),
        DataType::Struct(semantic_id) => concrete_type(semantic_id),
        DataType::Array(element) => applied_type("core.array", element),
        DataType::DataSeries(element) => applied_type("core.data_series", element),
        DataType::OneOf(values) => values
            .iter()
            .map(data_type_to_type_expr)
            .collect::<Result<Vec<_>, _>>()
            .map(TypeExpr::Union),
        DataType::Any => Ok(TypeExpr::Unknown),
    }
}

fn concrete_type(semantic_id: &str) -> Result<TypeExpr, String> {
    TypeId::new(semantic_id)
        .map(TypeExpr::Concrete)
        .map_err(|error| error.to_string())
}

fn applied_type(constructor: &str, element: &DataType) -> Result<TypeExpr, String> {
    Ok(TypeExpr::Applied {
        constructor: TypeConstructorId::new(constructor).map_err(|error| error.to_string())?,
        arguments: vec![data_type_to_type_expr(element)?],
    })
}

fn validate_scope(graph_path: &GraphResourcePath, protocol: &NodeProtocol) -> Result<(), String> {
    let scope = if graph_path.as_str().starts_with("events/") {
        yss_graph_protocol::NodeScope::Event
    } else if graph_path.as_str().starts_with("functions/") {
        yss_graph_protocol::NodeScope::Function
    } else {
        yss_graph_protocol::NodeScope::Any
    };
    if protocol.scope != yss_graph_protocol::NodeScope::Any
        && scope != yss_graph_protocol::NodeScope::Any
        && protocol.scope != scope
    {
        Err(format!(
            "node type '{}' is out of graph scope",
            protocol.type_id
        ))
    } else {
        Ok(())
    }
}

fn ports_are_compatible(source: &SourcePort, candidate: &CandidatePort) -> bool {
    if source.direction == candidate.direction || source.kind != candidate.kind {
        return false;
    }
    if source.kind != PortKind::Data {
        return true;
    }
    if type_expr_is_unresolved(&source.value_type, &source.type_parameters)
        || type_expr_is_unresolved(&candidate.value_type, &candidate.type_parameters)
    {
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
}
