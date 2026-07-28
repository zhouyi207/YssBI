use super::materialization::ProjectedMemberRef;
use super::{
    ConnectionId, DocumentConnection, DocumentError, DocumentNode, DynamicPortBinding,
    GraphDocument, GraphDocumentOperation, GraphDocumentPatch, GraphResourcePath,
    MaterializationAuthorization, NodeId, NodePosition, OperationId, OrderKey, ParameterValues,
    PortAddress, PortInstanceId, PortRef, ResourceKey, ResourceRevision, TypedValue,
};
use crate::node_system::protocol::{
    ConnectionsPerPort, LiteralPolicy, NodeProtocol, NodeScope, NodeTypeId, ParameterConstraint,
    PortDirection, PortInstances, PortKey, PortKind, PortSpec, TypeExpr, Value,
};
use crate::node_system::registry::NodeRegistry;

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// A client mutation paired with the resource revision it was based on.
///
/// `operation_id` is correlation metadata only. The store echoes it in the
/// committed event and does not use it for identity or deduplication.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationRequest<T> {
    pub resource: ResourceKey,
    pub base_revision: ResourceRevision,
    pub operation_id: OperationId,
    pub payload: T,
}

impl<T> MutationRequest<T> {
    pub const fn new(
        resource: ResourceKey,
        base_revision: ResourceRevision,
        operation_id: OperationId,
        payload: T,
    ) -> Self {
        Self {
            resource,
            base_revision,
            operation_id,
            payload,
        }
    }
}

/// A delta produced only after the corresponding mutation has committed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDeltaEvent<T> {
    pub graph_path: GraphResourcePath,
    pub from_revision: ResourceRevision,
    pub to_revision: ResourceRevision,
    pub caused_by: Option<OperationId>,
    pub payload: T,
}

impl<T> GraphDeltaEvent<T> {
    pub fn revision_gap_after(&self, applied_revision: ResourceRevision) -> Option<RevisionGap> {
        detect_revision_gap(applied_revision, self)
    }

    pub fn has_monotonic_revision(&self) -> bool {
        self.to_revision == self.from_revision.next()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionGap {
    pub expected_before_revision: ResourceRevision,
    pub actual_before_revision: ResourceRevision,
}

pub fn detect_revision_gap<T>(
    applied_revision: ResourceRevision,
    event: &GraphDeltaEvent<T>,
) -> Option<RevisionGap> {
    (event.from_revision != applied_revision).then_some(RevisionGap {
        expected_before_revision: applied_revision,
        actual_before_revision: event.from_revision,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum MutationConflict {
    RecoveryRequired(Box<str>),
    ResourceMismatch {
        requested: ResourceKey,
        store: ResourceKey,
    },
    StaleRevision {
        base_revision: ResourceRevision,
        current_revision: ResourceRevision,
    },
    CompilationBasisGraphMismatch {
        basis_graph_path: GraphResourcePath,
        store_graph_path: GraphResourcePath,
    },
    CompilationBasisStale {
        basis_revision: ResourceRevision,
        current_revision: ResourceRevision,
    },
    MaterializationUnauthorized,
    InvalidEditorMutation(Box<str>),
    Projection(Box<str>),
    History(Box<str>),
    Document(DocumentError),
}

impl MutationConflict {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::RecoveryRequired(_) => "project_recovery_required",
            _ => "mutation_conflict",
        }
    }
}

impl fmt::Display for MutationConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecoveryRequired(message) => formatter.write_str(message),
            Self::ResourceMismatch { requested, store } => write!(
                formatter,
                "mutation targets {requested:?}, but this store owns {store:?}"
            ),
            Self::StaleRevision {
                base_revision,
                current_revision,
            } => write!(
                formatter,
                "mutation base revision {} is stale; current revision is {}",
                base_revision.get(),
                current_revision.get()
            ),
            Self::CompilationBasisGraphMismatch {
                basis_graph_path,
                store_graph_path,
            } => write!(
                formatter,
                "compilation basis graph {basis_graph_path:?} does not match store graph {store_graph_path:?}"
            ),
            Self::CompilationBasisStale {
                basis_revision,
                current_revision,
            } => write!(
                formatter,
                "compilation basis revision {} is stale; current revision is {}",
                basis_revision.get(),
                current_revision.get()
            ),
            Self::MaterializationUnauthorized => {
                formatter.write_str("materialization authorization does not match projected member")
            }
            Self::InvalidEditorMutation(message) => formatter.write_str(message),
            Self::Projection(message) => {
                write!(formatter, "committed graph projection failed: {message}")
            }
            Self::History(message) => {
                write!(formatter, "project history transaction failed: {message}")
            }
            Self::Document(source) => write!(formatter, "mutation patch failed: {source}"),
        }
    }
}

impl std::error::Error for MutationConflict {}

impl From<DocumentError> for MutationConflict {
    fn from(source: DocumentError) -> Self {
        Self::Document(source)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PortAddressDto {
    Declared {
        node_id: Box<str>,
        port_key: Box<str>,
    },
    Instance {
        node_id: Box<str>,
        template_key: Box<str>,
        instance_id: Box<str>,
    },
}

impl From<&PortAddress> for PortAddressDto {
    fn from(address: &PortAddress) -> Self {
        match &address.port {
            PortRef::Declared { key } => Self::Declared {
                node_id: address.node_id.to_string().into(),
                port_key: key.as_str().into(),
            },
            PortRef::Instance {
                template,
                instance_id,
            } => Self::Instance {
                node_id: address.node_id.to_string().into(),
                template_key: template.as_str().into(),
                instance_id: instance_id.to_string().into(),
            },
        }
    }
}

impl From<PortAddress> for PortAddressDto {
    fn from(address: PortAddress) -> Self {
        Self::from(&address)
    }
}

impl TryFrom<PortAddressDto> for PortAddress {
    type Error = String;

    fn try_from(address: PortAddressDto) -> Result<Self, Self::Error> {
        match address {
            PortAddressDto::Declared { node_id, port_key } => Ok(Self::declared(
                parse_node_id(&node_id)?,
                PortKey::new(port_key).map_err(|error| error.to_string())?,
            )),
            PortAddressDto::Instance {
                node_id,
                template_key,
                instance_id,
            } => Ok(Self::instance(
                parse_node_id(&node_id)?,
                PortKey::new(template_key).map_err(|error| error.to_string())?,
                PortInstanceId::from_uuid(
                    uuid::Uuid::parse_str(&instance_id).map_err(|error| error.to_string())?,
                ),
            )),
        }
    }
}

fn parse_node_id(value: &str) -> Result<NodeId, String> {
    uuid::Uuid::parse_str(value)
        .map(NodeId::from_uuid)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EditorGraphMutationDto {
    CreateNode {
        node_type_id: NodeTypeId,
        position: NodePosition,
        parameters: ParameterValues,
        user_label: Option<String>,
    },
    DeleteNode {
        node_id: NodeId,
    },
    MoveNodes {
        positions: Vec<NodePositionMutationDto>,
    },
    Connect {
        output: PortAddressDto,
        input: PortAddressDto,
        order: Option<OrderKey>,
    },
    Disconnect {
        connection_id: ConnectionId,
    },
    SetLiteral {
        address: PortAddressDto,
        literal: Option<TypedValue>,
    },
    AddPortInstance {
        node_id: NodeId,
        template: PortKey,
        order: Option<OrderKey>,
    },
    RemovePortInstance {
        address: PortAddressDto,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePositionMutationDto {
    pub node_id: NodeId,
    pub position: NodePosition,
}

impl EditorGraphMutationDto {
    pub fn into_patch(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        registry: &NodeRegistry,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        let operations = match self {
            Self::CreateNode {
                node_type_id,
                position,
                parameters,
                user_label,
            } => {
                validate_position(position)?;
                let protocol = registry.protocol(&node_type_id).ok_or_else(|| {
                    invalid_editor_mutation(format!("unknown node type '{node_type_id}'"))
                })?;
                validate_node_scope(graph_path, protocol)?;
                validate_parameters(protocol, &parameters)?;
                create_node_operations(protocol, node_type_id, position, parameters, user_label)
            }
            Self::DeleteNode { node_id } => {
                delete_editor_node_operations(document, registry, node_id)?
            }
            Self::MoveNodes { positions } => move_node_operations(document, positions)?,
            Self::Connect {
                output,
                input,
                order,
            } => connect_operations(
                document,
                registry,
                output.try_into().map_err(invalid_editor_mutation)?,
                input.try_into().map_err(invalid_editor_mutation)?,
                order,
            )?,
            Self::Disconnect { connection_id } => {
                let connection = document
                    .connections
                    .get(&connection_id)
                    .cloned()
                    .ok_or(DocumentError::ConnectionNotFound(connection_id))?;
                vec![GraphDocumentOperation::RemoveConnection { connection }]
            }
            Self::SetLiteral { address, literal } => {
                let address = address.try_into().map_err(invalid_editor_mutation)?;
                validate_literal_target(document, registry, &address)?;
                let before = document.input_states.get(&address).cloned();
                vec![GraphDocumentOperation::SetInputState {
                    address,
                    before,
                    after: literal.map(|value| super::InputState {
                        literal_override: Some(value),
                    }),
                }]
            }
            Self::AddPortInstance {
                node_id,
                template,
                order,
            } => add_port_instance_operations(document, registry, node_id, template, order)?,
            Self::RemovePortInstance { address } => remove_port_instance_operations(
                document,
                registry,
                address.try_into().map_err(invalid_editor_mutation)?,
            )?,
        };
        Ok(GraphDocumentPatch::new(operations))
    }
}

fn invalid_editor_mutation(message: impl Into<Box<str>>) -> MutationConflict {
    MutationConflict::InvalidEditorMutation(message.into())
}

fn delete_editor_node_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    node_id: NodeId,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let node = document
        .nodes
        .get(&node_id)
        .ok_or(DocumentError::NodeNotFound(node_id))?;
    let protocol = registry.protocol(&node.node_type).ok_or_else(|| {
        invalid_editor_mutation(format!("unknown node type '{}'", node.node_type))
    })?;
    if protocol.managed_role.is_some() {
        return Err(invalid_editor_mutation(format!(
            "managed node '{}' cannot be deleted by an editor mutation",
            node.node_type
        )));
    }
    delete_node_operations(document, node_id).map_err(MutationConflict::from)
}

fn validate_node_scope(
    graph_path: &GraphResourcePath,
    protocol: &NodeProtocol,
) -> Result<(), MutationConflict> {
    let graph_scope = if graph_path.0.starts_with("events/") {
        NodeScope::Event
    } else if graph_path.0.starts_with("functions/") {
        NodeScope::Function
    } else {
        NodeScope::Any
    };
    if protocol.scope != NodeScope::Any
        && graph_scope != NodeScope::Any
        && protocol.scope != graph_scope
    {
        Err(invalid_editor_mutation(format!(
            "node scope {:?} is invalid for graph '{}'",
            protocol.scope, graph_path.0
        )))
    } else {
        Ok(())
    }
}

fn create_node_operations(
    protocol: &NodeProtocol,
    node_type: NodeTypeId,
    position: NodePosition,
    parameters: ParameterValues,
    user_label: Option<String>,
) -> Vec<GraphDocumentOperation> {
    let node_id = NodeId::new();
    let mut operations = vec![GraphDocumentOperation::InsertNode {
        node: DocumentNode {
            id: node_id,
            node_type,
            position,
            parameters,
            user_label,
        },
    }];
    for spec in protocol.interface.ports.iter() {
        let PortInstances::UserCreated { min, .. } = spec.instances else {
            continue;
        };
        for index in 0..min {
            let instance_id = PortInstanceId::new();
            operations.push(GraphDocumentOperation::InsertPortBinding {
                address: PortAddress::instance(node_id, spec.key.clone(), instance_id),
                binding: DynamicPortBinding::UserCreated {
                    order: OrderKey(format!("{index:05}").into()),
                },
            });
        }
    }
    operations
}

fn validate_position(position: NodePosition) -> Result<(), MutationConflict> {
    if position.x.is_finite() && position.y.is_finite() {
        Ok(())
    } else {
        Err(invalid_editor_mutation("node position must be finite"))
    }
}

fn validate_parameters(
    protocol: &NodeProtocol,
    parameters: &ParameterValues,
) -> Result<(), MutationConflict> {
    for key in parameters.keys() {
        if !protocol
            .parameters
            .parameters
            .iter()
            .any(|spec| &spec.key == key)
        {
            return Err(invalid_editor_mutation(format!(
                "unknown parameter '{key}' for node type '{}'",
                protocol.type_id
            )));
        }
    }
    for spec in protocol.parameters.parameters.iter() {
        let Some(value) = parameters.get(&spec.key) else {
            if spec.default_value.is_none()
                && spec.constraints.contains(&ParameterConstraint::Required)
            {
                return Err(invalid_editor_mutation(format!(
                    "required parameter '{}' is missing",
                    spec.key
                )));
            }
            continue;
        };
        if spec.constraints.contains(&ParameterConstraint::Required) && value.is_null() {
            return Err(invalid_editor_mutation(format!(
                "required parameter '{}' cannot be null",
                spec.key
            )));
        }
        if !parameter_value_matches_type(value, &spec.value_type) {
            return Err(invalid_editor_mutation(format!(
                "parameter '{}' does not match its declared type",
                spec.key
            )));
        }
        for constraint in &spec.constraints {
            validate_parameter_constraint(&spec.key, value, constraint)?;
        }
    }
    Ok(())
}

fn parameter_value_matches_type(value: &serde_json::Value, expected: &TypeExpr) -> bool {
    match expected {
        TypeExpr::Concrete(id) => match id.as_str() {
            "core.bool" => value.is_boolean(),
            "core.int64" => value.as_i64().is_some(),
            "core.float64" => value.is_number(),
            "core.string" => value.is_string(),
            _ => true,
        },
        TypeExpr::Union(options) => options
            .iter()
            .any(|option| parameter_value_matches_type(value, option)),
        TypeExpr::Generic(_) | TypeExpr::Applied { .. } | TypeExpr::Unknown => true,
    }
}

fn validate_parameter_constraint(
    key: &crate::node_system::protocol::ParameterKey,
    value: &serde_json::Value,
    constraint: &ParameterConstraint,
) -> Result<(), MutationConflict> {
    let valid = match constraint {
        ParameterConstraint::Required => !value.is_null(),
        ParameterConstraint::OneOf(options) => options
            .iter()
            .any(|option| protocol_value_matches_json(option, value)),
        ParameterConstraint::IntegerRange { min, max } => value.as_i64().is_some_and(|value| {
            min.is_none_or(|minimum| value >= minimum) && max.is_none_or(|maximum| value <= maximum)
        }),
        ParameterConstraint::Length { min, max } => parameter_length(value).is_some_and(|length| {
            min.is_none_or(|minimum| length >= minimum as usize)
                && max.is_none_or(|maximum| length <= maximum as usize)
        }),
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_editor_mutation(format!(
            "parameter '{key}' violates its protocol constraints"
        )))
    }
}

fn parameter_length(value: &serde_json::Value) -> Option<usize> {
    match value {
        serde_json::Value::String(value) => Some(value.chars().count()),
        serde_json::Value::Array(value) => Some(value.len()),
        serde_json::Value::Object(value) => Some(value.len()),
        _ => None,
    }
}

fn protocol_value_matches_json(expected: &Value, actual: &serde_json::Value) -> bool {
    match (expected, actual) {
        (Value::Null, serde_json::Value::Null) => true,
        (Value::Bool(expected), serde_json::Value::Bool(actual)) => expected == actual,
        (Value::Integer(expected), actual) => actual.as_i64() == Some(*expected),
        (Value::Unsigned(expected), actual) => actual.as_u64() == Some(*expected),
        (Value::Decimal(expected), serde_json::Value::String(actual)) => {
            expected.as_str() == actual
        }
        (Value::String(expected), serde_json::Value::String(actual)) => expected.as_ref() == actual,
        (Value::Bytes(expected), serde_json::Value::Array(actual)) => actual
            .iter()
            .map(serde_json::Value::as_u64)
            .eq(expected.iter().map(|byte| Some(u64::from(*byte)))),
        (Value::List(expected), serde_json::Value::Array(actual)) => {
            expected.len() == actual.len()
                && expected
                    .iter()
                    .zip(actual)
                    .all(|(expected, actual)| protocol_value_matches_json(expected, actual))
        }
        (Value::Object(expected), serde_json::Value::Object(actual)) => {
            expected.len() == actual.len()
                && expected.iter().all(|(key, expected)| {
                    actual
                        .get(key.as_ref())
                        .is_some_and(|actual| protocol_value_matches_json(expected, actual))
                })
        }
        _ => false,
    }
}

fn move_node_operations(
    document: &GraphDocument,
    positions: Vec<NodePositionMutationDto>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let mut seen = BTreeSet::new();
    for target in &positions {
        validate_position(target.position)?;
        if !seen.insert(target.node_id) {
            return Err(invalid_editor_mutation(format!(
                "node '{}' appears more than once in a move",
                target.node_id
            )));
        }
        if !document.nodes.contains_key(&target.node_id) {
            return Err(DocumentError::NodeNotFound(target.node_id).into());
        }
    }
    Ok(positions
        .into_iter()
        .map(|target| {
            let before = document.nodes[&target.node_id].clone();
            let mut after = before.clone();
            after.position = target.position;
            GraphDocumentOperation::UpdateNode { before, after }
        })
        .collect())
}

struct MutationPort<'a> {
    spec: &'a PortSpec,
    binding: Option<&'a DynamicPortBinding>,
}

fn resolve_mutation_port<'a>(
    document: &'a GraphDocument,
    registry: &'a NodeRegistry,
    address: &PortAddress,
) -> Result<MutationPort<'a>, MutationConflict> {
    let node = document
        .nodes
        .get(&address.node_id)
        .ok_or(DocumentError::EndpointNodeNotFound(address.node_id))?;
    let protocol = registry.protocol(&node.node_type).ok_or_else(|| {
        invalid_editor_mutation(format!("unknown node type '{}'", node.node_type))
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
        .ok_or_else(|| invalid_editor_mutation(format!("unknown port '{address}'")))?;
    let binding = match &address.port {
        PortRef::Declared { .. } => {
            if !matches!(spec.instances, PortInstances::Declared) {
                return Err(invalid_editor_mutation(format!(
                    "port '{address}' requires an instance address"
                )));
            }
            None
        }
        PortRef::Instance { .. } => {
            let binding = document
                .port_bindings
                .get(address)
                .ok_or_else(|| DocumentError::MissingPortBinding(address.clone()))?;
            let compatible = matches!(
                (&spec.instances, binding),
                (
                    PortInstances::UserCreated { .. },
                    DynamicPortBinding::UserCreated { .. }
                ) | (
                    PortInstances::Derived { .. },
                    DynamicPortBinding::Resolved { .. } | DynamicPortBinding::Orphan { .. }
                )
            );
            if !compatible {
                return Err(invalid_editor_mutation(format!(
                    "port binding kind does not match template '{address}'"
                )));
            }
            Some(binding)
        }
    };
    Ok(MutationPort { spec, binding })
}

fn connect_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let output_port = resolve_mutation_port(document, registry, &output)?;
    let input_port = resolve_mutation_port(document, registry, &input)?;
    if matches!(output_port.binding, Some(DynamicPortBinding::Orphan { .. }))
        || matches!(input_port.binding, Some(DynamicPortBinding::Orphan { .. }))
    {
        return Err(invalid_editor_mutation("orphan ports cannot be connected"));
    }
    if output_port.spec.direction != PortDirection::Output
        || input_port.spec.direction != PortDirection::Input
    {
        return Err(invalid_editor_mutation(
            "connection endpoints have invalid directions",
        ));
    }
    if output_port.spec.kind != input_port.spec.kind {
        return Err(invalid_editor_mutation(
            "connection endpoint kinds do not match",
        ));
    }
    validate_connection_capacity(document, &output, output_port.spec.connections)?;
    validate_connection_capacity(document, &input, input_port.spec.connections)?;
    match input_port.spec.connections {
        ConnectionsPerPort::Multiple { ordered: true, .. } if order.is_none() => {
            return Err(invalid_editor_mutation(
                "ordered input connections require an order key",
            ));
        }
        ConnectionsPerPort::Single | ConnectionsPerPort::Multiple { ordered: false, .. }
            if order.is_some() =>
        {
            return Err(invalid_editor_mutation(
                "unordered input connections cannot carry an order key",
            ));
        }
        _ => {}
    }
    Ok(vec![GraphDocumentOperation::InsertConnection {
        connection: DocumentConnection {
            id: ConnectionId::new(),
            output,
            input,
            order,
        },
    }])
}

fn validate_connection_capacity(
    document: &GraphDocument,
    address: &PortAddress,
    capability: ConnectionsPerPort,
) -> Result<(), MutationConflict> {
    let current = document
        .connections
        .values()
        .filter(|connection| connection.output == *address || connection.input == *address)
        .count();
    let maximum = match capability {
        ConnectionsPerPort::Single => Some(1usize),
        ConnectionsPerPort::Multiple { max, .. } => max.map(usize::from),
    };
    if maximum.is_some_and(|maximum| current >= maximum) {
        Err(invalid_editor_mutation(format!(
            "port '{address}' has reached its connection limit"
        )))
    } else {
        Ok(())
    }
}

fn validate_literal_target(
    document: &GraphDocument,
    registry: &NodeRegistry,
    address: &PortAddress,
) -> Result<(), MutationConflict> {
    let port = resolve_mutation_port(document, registry, address)?;
    if matches!(port.binding, Some(DynamicPortBinding::Orphan { .. })) {
        return Err(invalid_editor_mutation(
            "orphan ports cannot carry literal overrides",
        ));
    }
    if port.spec.direction != PortDirection::Input || port.spec.kind != PortKind::Data {
        return Err(invalid_editor_mutation(
            "literal overrides require a data input port",
        ));
    }
    if !matches!(
        port.spec
            .input_binding
            .as_ref()
            .map(|binding| binding.literal_policy),
        Some(LiteralPolicy::Allowed)
    ) {
        return Err(invalid_editor_mutation(
            "the input protocol forbids literal overrides",
        ));
    }
    Ok(())
}

fn add_port_instance_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    node_id: NodeId,
    template: PortKey,
    order: Option<OrderKey>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let spec = user_created_port_spec(document, registry, node_id, &template)?;
    let count = user_created_instance_count(document, node_id, &template);
    let PortInstances::UserCreated { max, .. } = spec.instances else {
        unreachable!("user_created_port_spec guarantees the instance policy")
    };
    if max.is_some_and(|maximum| count >= usize::from(maximum)) {
        return Err(invalid_editor_mutation(format!(
            "port template '{template}' has reached its maximum instance count"
        )));
    }
    let instance_id = PortInstanceId::new();
    let order = order.unwrap_or_else(|| OrderKey(instance_id.to_string().into()));
    Ok(vec![GraphDocumentOperation::InsertPortBinding {
        address: PortAddress::instance(node_id, template, instance_id),
        binding: DynamicPortBinding::UserCreated { order },
    }])
}

fn remove_port_instance_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    address: PortAddress,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let PortRef::Instance { template, .. } = &address.port else {
        return Err(invalid_editor_mutation(
            "only instance ports can be removed",
        ));
    };
    let spec = user_created_port_spec(document, registry, address.node_id, template)?;
    let binding = document
        .port_bindings
        .get(&address)
        .cloned()
        .ok_or_else(|| DocumentError::PortBindingNotFound(address.clone()))?;
    if !matches!(binding, DynamicPortBinding::UserCreated { .. }) {
        return Err(invalid_editor_mutation(
            "only user-created port instances can be removed",
        ));
    }
    let PortInstances::UserCreated { min, .. } = spec.instances else {
        unreachable!("user_created_port_spec guarantees the instance policy")
    };
    let count = user_created_instance_count(document, address.node_id, template);
    if count <= usize::from(min) {
        return Err(invalid_editor_mutation(format!(
            "port template '{template}' requires at least {min} instances"
        )));
    }
    let mut operations = document
        .connections
        .values()
        .filter(|connection| connection.output == address || connection.input == address)
        .cloned()
        .map(|connection| GraphDocumentOperation::RemoveConnection { connection })
        .collect::<Vec<_>>();
    if let Some(before) = document.input_states.get(&address).cloned() {
        operations.push(GraphDocumentOperation::SetInputState {
            address: address.clone(),
            before: Some(before),
            after: None,
        });
    }
    operations.push(GraphDocumentOperation::RemovePortBinding { address, binding });
    Ok(operations)
}

fn user_created_port_spec<'a>(
    document: &'a GraphDocument,
    registry: &'a NodeRegistry,
    node_id: NodeId,
    template: &PortKey,
) -> Result<&'a PortSpec, MutationConflict> {
    let node = document
        .nodes
        .get(&node_id)
        .ok_or(DocumentError::NodeNotFound(node_id))?;
    let protocol = registry.protocol(&node.node_type).ok_or_else(|| {
        invalid_editor_mutation(format!("unknown node type '{}'", node.node_type))
    })?;
    let spec = protocol
        .interface
        .ports
        .iter()
        .find(|spec| &spec.key == template)
        .ok_or_else(|| {
            invalid_editor_mutation(format!(
                "node '{node_id}' does not own port template '{template}'"
            ))
        })?;
    if !matches!(spec.instances, PortInstances::UserCreated { .. }) {
        return Err(invalid_editor_mutation(format!(
            "port template '{template}' is not user-created"
        )));
    }
    Ok(spec)
}

fn user_created_instance_count(
    document: &GraphDocument,
    node_id: NodeId,
    template: &PortKey,
) -> usize {
    document
        .port_bindings
        .iter()
        .filter(|(address, binding)| {
            address.node_id == node_id
                && matches!(&address.port, PortRef::Instance { template: current, .. } if current == template)
                && matches!(binding, DynamicPortBinding::UserCreated { .. })
        })
        .count()
}

/// Mutations accepted by the authoritative graph store.
///
/// The projected-member variant intentionally has no `PortInstanceId` field:
/// its durable instance identity is allocated while the store builds the
/// atomic materialize-and-connect patch.
#[derive(Debug)]
pub enum GraphMutation {
    CreateNode {
        node: DocumentNode,
    },
    DeleteNode {
        node_id: NodeId,
    },
    Connect {
        output: PortAddress,
        input: PortAddress,
        order: Option<OrderKey>,
    },
    Disconnect {
        connection_id: ConnectionId,
    },
    SetLiteral {
        address: PortAddress,
        literal: Option<TypedValue>,
    },
    MaterializeProjectedMemberAndConnect {
        member: ProjectedMemberRef,
        authorization: MaterializationAuthorization,
        output: PortAddress,
        order: Option<OrderKey>,
    },
}

impl GraphMutation {
    fn into_patch(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        let operations = match self {
            Self::CreateNode { node } => {
                vec![GraphDocumentOperation::InsertNode { node }]
            }
            Self::DeleteNode { node_id } => delete_node_operations(document, node_id)?,
            Self::Connect {
                output,
                input,
                order,
            } => vec![GraphDocumentOperation::InsertConnection {
                connection: DocumentConnection {
                    id: ConnectionId::new(),
                    output,
                    input,
                    order,
                },
            }],
            Self::Disconnect { connection_id } => {
                let connection = document
                    .connections
                    .get(&connection_id)
                    .cloned()
                    .ok_or(DocumentError::ConnectionNotFound(connection_id))?;
                vec![GraphDocumentOperation::RemoveConnection { connection }]
            }
            Self::SetLiteral { address, literal } => {
                let before = document.input_states.get(&address).cloned();
                vec![GraphDocumentOperation::SetInputState {
                    address,
                    before,
                    after: literal.map(|value| super::InputState {
                        literal_override: Some(value),
                    }),
                }]
            }
            Self::MaterializeProjectedMemberAndConnect {
                member,
                authorization,
                output,
                order,
            } => materialize_projected_member_operations(
                graph_path,
                document,
                member,
                authorization,
                output,
                order,
            )?,
        };
        Ok(GraphDocumentPatch::new(operations))
    }
}

fn materialize_projected_member_operations(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    member: ProjectedMemberRef,
    authorization: MaterializationAuthorization,
    output: PortAddress,
    order: Option<OrderKey>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    if authorization.member() != &member {
        return Err(MutationConflict::MaterializationUnauthorized);
    }
    if member.basis().graph_path() != graph_path {
        return Err(MutationConflict::CompilationBasisGraphMismatch {
            basis_graph_path: member.basis().graph_path().clone(),
            store_graph_path: graph_path.clone(),
        });
    }
    if member.basis().graph_revision() != document.revision {
        return Err(MutationConflict::CompilationBasisStale {
            basis_revision: member.basis().graph_revision(),
            current_revision: document.revision,
        });
    }

    let input = PortAddress::instance(
        member.node_id(),
        member.template().clone(),
        PortInstanceId::new(),
    );
    Ok(vec![
        GraphDocumentOperation::InsertPortBinding {
            address: input.clone(),
            binding: authorization.into_binding(),
        },
        GraphDocumentOperation::InsertConnection {
            connection: DocumentConnection {
                id: ConnectionId::new(),
                output,
                input,
                order,
            },
        },
    ])
}

fn delete_node_operations(
    document: &GraphDocument,
    node_id: NodeId,
) -> Result<Vec<GraphDocumentOperation>, DocumentError> {
    let node = document
        .nodes
        .get(&node_id)
        .cloned()
        .ok_or(DocumentError::NodeNotFound(node_id))?;
    let mut operations = Vec::new();
    operations.extend(
        document
            .connections
            .values()
            .filter(|connection| {
                connection.output.node_id == node_id || connection.input.node_id == node_id
            })
            .cloned()
            .map(|connection| GraphDocumentOperation::RemoveConnection { connection }),
    );
    operations.extend(
        document
            .input_states
            .iter()
            .filter(|(address, _)| address.node_id == node_id)
            .map(|(address, state)| GraphDocumentOperation::SetInputState {
                address: address.clone(),
                before: Some(state.clone()),
                after: None,
            }),
    );
    operations.extend(
        document
            .port_bindings
            .iter()
            .filter(|(address, _)| address.node_id == node_id)
            .map(
                |(address, binding)| GraphDocumentOperation::RemovePortBinding {
                    address: address.clone(),
                    binding: binding.clone(),
                },
            ),
    );
    operations.push(GraphDocumentOperation::RemoveNode { node });
    Ok(operations)
}

/// Authoritative in-memory wrapper for one graph document.
///
/// Mutation planning and patch application are pure in-memory operations, so
/// callers never need to hold this authority across filesystem or network I/O.
#[derive(Debug, Clone)]
pub struct RevisionedGraphStore {
    graph_path: GraphResourcePath,
    document: GraphDocument,
}

impl RevisionedGraphStore {
    pub fn new(graph_path: GraphResourcePath, document: GraphDocument) -> Self {
        Self {
            graph_path,
            document,
        }
    }

    pub const fn graph_path(&self) -> &GraphResourcePath {
        &self.graph_path
    }

    pub const fn document(&self) -> &GraphDocument {
        &self.document
    }

    pub const fn revision(&self) -> ResourceRevision {
        self.document.revision
    }

    pub fn into_document(self) -> GraphDocument {
        self.document
    }

    pub fn apply_mutation(
        &mut self,
        request: MutationRequest<GraphMutation>,
    ) -> Result<GraphDeltaEvent<GraphDocumentPatch>, MutationConflict> {
        let store_resource = ResourceKey::Graph(self.graph_path.clone());
        if request.resource != store_resource {
            return Err(MutationConflict::ResourceMismatch {
                requested: request.resource,
                store: store_resource,
            });
        }
        if request.base_revision != self.document.revision {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision,
                current_revision: self.document.revision,
            });
        }

        let from_revision = self.document.revision;
        let patch = request
            .payload
            .into_patch(&self.graph_path, &self.document)?;
        self.document.apply_patch(&patch)?;

        Ok(GraphDeltaEvent {
            graph_path: self.graph_path.clone(),
            from_revision,
            to_revision: self.document.revision,
            caused_by: Some(request.operation_id),
            payload: patch,
        })
    }
}

pub fn apply_mutation(
    store: &mut RevisionedGraphStore,
    request: MutationRequest<GraphMutation>,
) -> Result<GraphDeltaEvent<GraphDocumentPatch>, MutationConflict> {
    store.apply_mutation(request)
}
