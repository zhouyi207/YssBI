use super::mutation::{
    validate_literal_target, validate_node_scope, validate_parameters_with_registry,
    validate_resolved_dynamic_binding_authority, validate_subgraph_connection,
    validate_subgraph_port,
};
use super::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionParameterId, GraphDocument, GraphDocumentOperation, GraphDocumentPatch,
    GraphResourcePath, InputState, LastKnownPortMetadata, MutationConflict, NodeId, NodePosition,
    OrderKey, ParameterValues, PortAddress, PortInstanceId, PortRef, SchemaFieldIdentity,
    SchemaSourceIdentity,
};
use crate::node_system::catalog::{
    CatalogResourcePath, NodeCreationDescriptor, ResourceBoundCreateArgsDto,
    authoritative_static_descriptor,
};
use crate::node_system::protocol::{
    NodeInstanceDisplaySpec, NodeTypeId, ParameterKey, PortInstances, PortKey, ResourceDisplayKind,
    TypeExpr,
};
use crate::node_system::registry::NodeRegistry;
use crate::project::{CatalogMutationResource, CatalogMutationValidationSnapshot};
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const CLIPBOARD_SUBGRAPH_SCHEMA_VERSION: u32 = 1;
pub const MAX_CLIPBOARD_NODES: usize = 500;
pub const MAX_CLIPBOARD_CONNECTIONS: usize = 2_000;
pub const MAX_CLIPBOARD_PORT_BINDINGS: usize = 4_000;
pub const MAX_CLIPBOARD_INPUT_STATES: usize = 4_000;
pub const MAX_CLIPBOARD_PARAMETER_BYTES: usize = 1_048_576;
pub const MAX_CLIPBOARD_VALUE_DEPTH: usize = 64;
pub const MAX_CLIPBOARD_SERIALIZED_BYTES: usize = 4_194_304;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClipboardNodeId(pub Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClipboardPortInstanceId(pub Box<str>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClipboardNodeCreationDto {
    Static {
        node_type_id: NodeTypeId,
    },
    ResourceBound {
        node_type_id: NodeTypeId,
        resource_path: CatalogResourcePath,
        create_args: ResourceBoundCreateArgsDto,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClipboardPortRefDto {
    Declared {
        key: PortKey,
    },
    Instance {
        template: PortKey,
        local_instance_id: ClipboardPortInstanceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardPortAddressDto {
    pub node_id: ClipboardNodeId,
    pub port: ClipboardPortRefDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardNodeDto {
    pub local_id: ClipboardNodeId,
    pub creation: ClipboardNodeCreationDto,
    #[serde(deserialize_with = "deserialize_parameter_values")]
    pub parameters: ParameterValues,
    #[serde(deserialize_with = "deserialize_bounded_optional_string")]
    pub user_label: Option<String>,
    pub relative_position: NodePosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClipboardDynamicMemberOriginDto {
    FunctionParameter {
        function: GraphResourcePath,
        parameter: FunctionParameterId,
    },
    SchemaField {
        source: SchemaSourceIdentity,
        field: SchemaFieldIdentity,
    },
}

impl From<&DynamicMemberLocator> for ClipboardDynamicMemberOriginDto {
    fn from(origin: &DynamicMemberLocator) -> Self {
        match origin {
            DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } => Self::FunctionParameter {
                function: function.clone(),
                parameter: parameter.clone(),
            },
            DynamicMemberLocator::SchemaField { source, field } => Self::SchemaField {
                source: source.clone(),
                field: field.clone(),
            },
        }
    }
}

impl From<ClipboardDynamicMemberOriginDto> for DynamicMemberLocator {
    fn from(origin: ClipboardDynamicMemberOriginDto) -> Self {
        match origin {
            ClipboardDynamicMemberOriginDto::FunctionParameter {
                function,
                parameter,
            } => Self::FunctionParameter {
                function,
                parameter,
            },
            ClipboardDynamicMemberOriginDto::SchemaField { source, field } => {
                Self::SchemaField { source, field }
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardLastKnownPortMetadataDto {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<TypeExpr>,
}

impl From<&LastKnownPortMetadata> for ClipboardLastKnownPortMetadataDto {
    fn from(last_known: &LastKnownPortMetadata) -> Self {
        Self {
            label: last_known.label.clone(),
            value_type: last_known.value_type.clone(),
        }
    }
}

impl From<ClipboardLastKnownPortMetadataDto> for LastKnownPortMetadata {
    fn from(last_known: ClipboardLastKnownPortMetadataDto) -> Self {
        Self {
            label: last_known.label,
            value_type: last_known.value_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClipboardDynamicPortBindingDto {
    UserCreated {
        order: OrderKey,
    },
    Resolved {
        origin: ClipboardDynamicMemberOriginDto,
        order: OrderKey,
        last_known: ClipboardLastKnownPortMetadataDto,
    },
    Orphan {
        origin: ClipboardDynamicMemberOriginDto,
        order: OrderKey,
        last_known: ClipboardLastKnownPortMetadataDto,
    },
}

impl From<&DynamicPortBinding> for ClipboardDynamicPortBindingDto {
    fn from(binding: &DynamicPortBinding) -> Self {
        match binding {
            DynamicPortBinding::UserCreated { order } => Self::UserCreated {
                order: order.clone(),
            },
            DynamicPortBinding::Resolved {
                origin,
                order,
                last_known,
            } => Self::Resolved {
                origin: origin.into(),
                order: order.clone(),
                last_known: last_known.into(),
            },
            DynamicPortBinding::Orphan {
                origin,
                order,
                last_known,
            } => Self::Orphan {
                origin: origin.into(),
                order: order.clone(),
                last_known: last_known.into(),
            },
        }
    }
}

impl From<ClipboardDynamicPortBindingDto> for DynamicPortBinding {
    fn from(binding: ClipboardDynamicPortBindingDto) -> Self {
        match binding {
            ClipboardDynamicPortBindingDto::UserCreated { order } => Self::UserCreated { order },
            ClipboardDynamicPortBindingDto::Resolved {
                origin,
                order,
                last_known,
            } => Self::Resolved {
                origin: origin.into(),
                order,
                last_known: last_known.into(),
            },
            ClipboardDynamicPortBindingDto::Orphan {
                origin,
                order,
                last_known,
            } => Self::Orphan {
                origin: origin.into(),
                order,
                last_known: last_known.into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardPortBindingDto {
    pub address: ClipboardPortAddressDto,
    pub binding: ClipboardDynamicPortBindingDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardInputStateDto {
    pub address: ClipboardPortAddressDto,
    #[serde(
        serialize_with = "serialize_clipboard_input_state",
        deserialize_with = "deserialize_bounded_input_state"
    )]
    pub state: InputState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardConnectionDto {
    pub output: ClipboardPortAddressDto,
    pub input: ClipboardPortAddressDto,
    pub order: Option<OrderKey>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardSubgraphDto {
    pub schema_version: u32,
    pub nodes: Vec<ClipboardNodeDto>,
    pub port_bindings: Vec<ClipboardPortBindingDto>,
    pub input_states: Vec<ClipboardInputStateDto>,
    pub connections: Vec<ClipboardConnectionDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClipboardSubgraphWire {
    schema_version: u32,
    #[serde(deserialize_with = "deserialize_nodes")]
    nodes: Vec<ClipboardNodeDto>,
    #[serde(deserialize_with = "deserialize_port_bindings")]
    port_bindings: Vec<ClipboardPortBindingDto>,
    #[serde(deserialize_with = "deserialize_input_states")]
    input_states: Vec<ClipboardInputStateDto>,
    #[serde(deserialize_with = "deserialize_connections")]
    connections: Vec<ClipboardConnectionDto>,
}

impl From<ClipboardSubgraphWire> for ClipboardSubgraphDto {
    fn from(wire: ClipboardSubgraphWire) -> Self {
        Self {
            schema_version: wire.schema_version,
            nodes: wire.nodes,
            port_bindings: wire.port_bindings,
            input_states: wire.input_states,
            connections: wire.connections,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ValidatedClipboardSubgraph(ClipboardSubgraphDto);

/// The only allowed decoder for untrusted clipboard JSON.
///
/// `InsertSubgraph` carries raw JSON and must cross this byte-limited boundary before
/// instantiation. The validated value is crate-private so production callers outside the
/// document module cannot bypass this decoder with a typed DTO.
pub(crate) fn deserialize_clipboard_subgraph(
    bytes: &[u8],
) -> Result<ValidatedClipboardSubgraph, MutationConflict> {
    if bytes.len() > MAX_CLIPBOARD_SERIALIZED_BYTES {
        return Err(invalid_clipboard(format!(
            "clipboard payload byte limit exceeded ({} > {})",
            bytes.len(),
            MAX_CLIPBOARD_SERIALIZED_BYTES
        )));
    }
    serde_json::from_slice::<ClipboardSubgraphWire>(bytes)
        .map(|wire| ValidatedClipboardSubgraph(wire.into()))
        .map_err(|error| invalid_clipboard(format!("clipboard payload is invalid: {error}")))
}

fn deserialize_nodes<'de, D>(deserializer: D) -> Result<Vec<ClipboardNodeDto>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec(deserializer, MAX_CLIPBOARD_NODES, "clipboard nodes")
}

fn deserialize_port_bindings<'de, D>(
    deserializer: D,
) -> Result<Vec<ClipboardPortBindingDto>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec(
        deserializer,
        MAX_CLIPBOARD_PORT_BINDINGS,
        "clipboard port bindings",
    )
}

fn deserialize_input_states<'de, D>(
    deserializer: D,
) -> Result<Vec<ClipboardInputStateDto>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec(
        deserializer,
        MAX_CLIPBOARD_INPUT_STATES,
        "clipboard input states",
    )
}

fn deserialize_connections<'de, D>(deserializer: D) -> Result<Vec<ClipboardConnectionDto>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec(
        deserializer,
        MAX_CLIPBOARD_CONNECTIONS,
        "clipboard connections",
    )
}

fn deserialize_bounded_vec<'de, D, T>(
    deserializer: D,
    limit: usize,
    name: &'static str,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct BoundedVecVisitor<T> {
        limit: usize,
        name: &'static str,
        marker: std::marker::PhantomData<T>,
    }

    impl<'de, T> Visitor<'de> for BoundedVecVisitor<T>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "{} with at most {} entries",
                self.name, self.limit
            )
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            if sequence.size_hint().is_some_and(|size| size > self.limit) {
                return Err(serde::de::Error::custom(format!(
                    "{} exceeds entry limit {}",
                    self.name, self.limit
                )));
            }
            let mut values = Vec::new();
            loop {
                if values.len() == self.limit {
                    return match sequence.next_element::<IgnoredAny>()? {
                        Some(_) => Err(serde::de::Error::custom(format!(
                            "{} exceeds entry limit {}",
                            self.name, self.limit
                        ))),
                        None => Ok(values),
                    };
                }
                match sequence.next_element()? {
                    Some(value) => values.push(value),
                    None => return Ok(values),
                }
            }
        }
    }

    deserializer.deserialize_seq(BoundedVecVisitor {
        limit,
        name,
        marker: std::marker::PhantomData,
    })
}

fn deserialize_parameter_values<'de, D>(deserializer: D) -> Result<ParameterValues, D::Error>
where
    D: Deserializer<'de>,
{
    struct ParameterValuesVisitor;

    impl<'de> Visitor<'de> for ParameterValuesVisitor {
        type Value = ParameterValues;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded clipboard parameter map")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut values = ParameterValues::new();
            while let Some(key) = map.next_key::<ParameterKey>()? {
                if values.len() >= MAX_CLIPBOARD_PORT_BINDINGS {
                    return Err(serde::de::Error::custom(
                        "clipboard parameter map exceeds entry limit",
                    ));
                }
                let value = map.next_value_seed(BoundedValueSeed { depth: 1 })?;
                if values.insert(key.clone(), value).is_some() {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate clipboard parameter '{key}'"
                    )));
                }
            }
            Ok(values)
        }
    }

    deserializer.deserialize_map(ParameterValuesVisitor)
}

fn deserialize_bounded_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    if value
        .as_ref()
        .is_some_and(|value| value.len() > MAX_CLIPBOARD_SERIALIZED_BYTES)
    {
        Err(serde::de::Error::custom(
            "clipboard string exceeds byte limit",
        ))
    } else {
        Ok(value)
    }
}

fn serialize_clipboard_input_state<S>(state: &InputState, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Wire<'a> {
        literal_override: &'a Option<serde_json::Value>,
    }

    Wire {
        literal_override: &state.literal_override,
    }
    .serialize(serializer)
}

fn deserialize_bounded_input_state<'de, D>(deserializer: D) -> Result<InputState, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Wire {
        #[serde(deserialize_with = "deserialize_bounded_optional_value")]
        literal_override: Option<serde_json::Value>,
    }

    let wire = Wire::deserialize(deserializer)?;
    Ok(InputState {
        literal_override: wire.literal_override,
    })
}

fn deserialize_bounded_optional_value<'de, D>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OptionalValueVisitor;

    impl<'de> Visitor<'de> for OptionalValueVisitor {
        type Value = Option<serde_json::Value>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a null or bounded JSON value")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            BoundedValueSeed { depth: 1 }
                .deserialize(deserializer)
                .map(Some)
        }
    }

    deserializer.deserialize_option(OptionalValueVisitor)
}

struct BoundedValueSeed {
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for BoundedValueSeed {
    type Value = serde_json::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.depth > MAX_CLIPBOARD_VALUE_DEPTH {
            return Err(serde::de::Error::custom(
                "clipboard JSON value exceeds depth limit",
            ));
        }
        deserializer.deserialize_any(BoundedValueVisitor { depth: self.depth })
    }
}

struct BoundedValueVisitor {
    depth: usize,
}

impl<'de> Visitor<'de> for BoundedValueVisitor {
    type Value = serde_json::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| E::custom("clipboard JSON number must be finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value.len() > MAX_CLIPBOARD_PARAMETER_BYTES {
            Err(E::custom("clipboard JSON string exceeds byte limit"))
        } else {
            Ok(serde_json::Value::String(value.to_owned()))
        }
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value.len() > MAX_CLIPBOARD_PARAMETER_BYTES {
            Err(E::custom("clipboard JSON string exceeds byte limit"))
        } else {
            Ok(serde_json::Value::String(value))
        }
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        BoundedValueSeed { depth: self.depth }.deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(BoundedValueSeed {
            depth: self.depth + 1,
        })? {
            if values.len() >= MAX_CLIPBOARD_PARAMETER_BYTES {
                return Err(serde::de::Error::custom(
                    "clipboard JSON array exceeds entry limit",
                ));
            }
            values.push(value);
        }
        Ok(serde_json::Value::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if key.len() > MAX_CLIPBOARD_PARAMETER_BYTES
                || values.len() >= MAX_CLIPBOARD_PARAMETER_BYTES
            {
                return Err(serde::de::Error::custom(
                    "clipboard JSON object exceeds limit",
                ));
            }
            let value = map.next_value_seed(BoundedValueSeed {
                depth: self.depth + 1,
            })?;
            if values.insert(key.clone(), value).is_some() {
                return Err(serde::de::Error::custom(format!(
                    "duplicate clipboard JSON key '{key}'"
                )));
            }
        }
        Ok(serde_json::Value::Object(values))
    }
}

pub fn export_subgraph(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, MutationConflict> {
    let selected = validate_targets(document, node_ids)?;
    enforce_limit("nodes", selected.len(), MAX_CLIPBOARD_NODES)?;

    let node_ids = selected.iter().copied().collect::<Vec<_>>();
    let local_nodes = node_ids
        .iter()
        .enumerate()
        .map(|(index, node_id)| (*node_id, ClipboardNodeId(format!("node/{index}").into())))
        .collect::<BTreeMap<_, _>>();
    let min_x = node_ids
        .iter()
        .map(|node_id| document.nodes[node_id].position.x)
        .reduce(f64::min)
        .expect("validated targets are non-empty");
    let min_y = node_ids
        .iter()
        .map(|node_id| document.nodes[node_id].position.y)
        .reduce(f64::min)
        .expect("validated targets are non-empty");

    let selected_bindings = document
        .port_bindings
        .iter()
        .filter(|(address, _)| selected.contains(&address.node_id))
        .collect::<Vec<_>>();
    let selected_states = document
        .input_states
        .iter()
        .filter(|(address, _)| selected.contains(&address.node_id))
        .collect::<Vec<_>>();
    let selected_connections = document
        .connections
        .values()
        .filter(|connection| {
            selected.contains(&connection.output.node_id)
                && selected.contains(&connection.input.node_id)
        })
        .collect::<Vec<_>>();
    enforce_limit(
        "port bindings",
        selected_bindings.len(),
        MAX_CLIPBOARD_PORT_BINDINGS,
    )?;
    enforce_limit(
        "input states",
        selected_states.len(),
        MAX_CLIPBOARD_INPUT_STATES,
    )?;
    enforce_limit(
        "connections",
        selected_connections.len(),
        MAX_CLIPBOARD_CONNECTIONS,
    )?;

    let local_instances = local_instance_ids(
        selected_bindings.iter().map(|(address, _)| *address),
        selected_states.iter().map(|(address, _)| *address),
        selected_connections
            .iter()
            .flat_map(|connection| [&connection.output, &connection.input]),
    );

    let mut parameter_bytes = 0usize;
    let mut nodes = Vec::with_capacity(node_ids.len());
    for node_id in node_ids {
        let node = &document.nodes[&node_id];
        validate_parameter_values(&node.parameters, &mut parameter_bytes)?;
        let creation = authoritative_creation(graph_path, node, registry, catalog)?;
        nodes.push(ClipboardNodeDto {
            local_id: local_nodes[&node_id].clone(),
            creation,
            parameters: node.parameters.clone(),
            user_label: node.user_label.clone(),
            relative_position: NodePosition {
                x: node.position.x - min_x,
                y: node.position.y - min_y,
            },
        });
    }

    let mut port_bindings = selected_bindings
        .into_iter()
        .map(|(address, binding)| {
            Ok(ClipboardPortBindingDto {
                address: rewrite_address(address, &local_nodes, &local_instances)?,
                binding: binding.into(),
            })
        })
        .collect::<Result<Vec<_>, MutationConflict>>()?;
    port_bindings.sort_by(|left, right| left.address.cmp(&right.address));

    let mut input_states = selected_states
        .into_iter()
        .map(|(address, state)| {
            Ok(ClipboardInputStateDto {
                address: rewrite_address(address, &local_nodes, &local_instances)?,
                state: state.clone(),
            })
        })
        .collect::<Result<Vec<_>, MutationConflict>>()?;
    input_states.sort_by(|left, right| left.address.cmp(&right.address));

    let mut connections = selected_connections
        .into_iter()
        .map(|connection| {
            Ok(ClipboardConnectionDto {
                output: rewrite_address(&connection.output, &local_nodes, &local_instances)?,
                input: rewrite_address(&connection.input, &local_nodes, &local_instances)?,
                order: connection.order.clone(),
            })
        })
        .collect::<Result<Vec<_>, MutationConflict>>()?;
    connections.sort_by(|left, right| {
        (&left.output, &left.input, &left.order).cmp(&(&right.output, &right.input, &right.order))
    });

    let snapshot = ClipboardSubgraphDto {
        schema_version: CLIPBOARD_SUBGRAPH_SCHEMA_VERSION,
        nodes,
        port_bindings,
        input_states,
        connections,
    };
    let serialized_bytes = serde_json::to_vec(&snapshot)
        .map_err(|error| invalid_export(format!("subgraph serialization failed: {error}")))?
        .len();
    enforce_limit(
        "serialized bytes",
        serialized_bytes,
        MAX_CLIPBOARD_SERIALIZED_BYTES,
    )?;
    Ok(snapshot)
}

pub fn duplicate_subgraph(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    node_ids: Vec<NodeId>,
    offset: NodePosition,
) -> Result<GraphDocumentPatch, MutationConflict> {
    let snapshot = export_subgraph(graph_path, document, registry, catalog, node_ids.clone())?;
    let origin_x = node_ids
        .iter()
        .map(|node_id| document.nodes[node_id].position.x)
        .reduce(f64::min)
        .expect("subgraph export validates non-empty targets");
    let origin_y = node_ids
        .iter()
        .map(|node_id| document.nodes[node_id].position.y)
        .reduce(f64::min)
        .expect("subgraph export validates non-empty targets");
    instantiate_subgraph(
        graph_path,
        document,
        registry,
        catalog,
        ValidatedClipboardSubgraph(snapshot),
        NodePosition {
            x: origin_x + offset.x,
            y: origin_y + offset.y,
        },
    )
}

pub(crate) fn instantiate_subgraph(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    snapshot: ValidatedClipboardSubgraph,
    anchor: NodePosition,
) -> Result<GraphDocumentPatch, MutationConflict> {
    let ValidatedClipboardSubgraph(mut snapshot) = snapshot;
    validate_insert_budget(&snapshot)?;
    snapshot
        .nodes
        .sort_by(|left, right| left.local_id.cmp(&right.local_id));
    snapshot
        .port_bindings
        .sort_by(|left, right| left.address.cmp(&right.address));
    snapshot
        .input_states
        .sort_by(|left, right| left.address.cmp(&right.address));
    snapshot.connections.sort_by(|left, right| {
        (&left.output, &left.input, &left.order).cmp(&(&right.output, &right.input, &right.order))
    });

    let node_types = validate_insert_nodes(graph_path, registry, catalog, &snapshot, anchor)?;
    let instance_keys = validate_portable_references(registry, catalog, &snapshot, &node_types)?;

    let temporary_nodes = temporary_node_ids(document, node_types.keys());
    let temporary_instances = temporary_port_instance_ids(document, instance_keys.iter());
    let temporary_connections = temporary_connection_ids(document, snapshot.connections.len());
    plan_instantiation(
        document,
        registry,
        &snapshot,
        anchor,
        &temporary_nodes,
        &temporary_instances,
        &temporary_connections,
    )?;

    let node_ids = fresh_node_ids(document, node_types.keys());
    let instance_ids = fresh_port_instance_ids(document, instance_keys.iter());
    let connection_ids = fresh_connection_ids(document, snapshot.connections.len());
    let patch = plan_instantiation(
        document,
        registry,
        &snapshot,
        anchor,
        &node_ids,
        &instance_ids,
        &connection_ids,
    )?;
    let mut staged = document.clone();
    patch
        .apply_without_revision(&mut staged)
        .map_err(|error| invalid_clipboard(format!("subgraph patch validation failed: {error}")))?;
    Ok(patch)
}

#[cfg(test)]
pub(crate) fn instantiate_subgraph_for_test(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    snapshot: ClipboardSubgraphDto,
    anchor: NodePosition,
) -> Result<GraphDocumentPatch, MutationConflict> {
    instantiate_subgraph(
        graph_path,
        document,
        registry,
        catalog,
        ValidatedClipboardSubgraph(snapshot),
        anchor,
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LocalInstanceKey {
    node_id: ClipboardNodeId,
    scope: PortKey,
    local_instance_id: ClipboardPortInstanceId,
}

fn validate_insert_budget(snapshot: &ClipboardSubgraphDto) -> Result<(), MutationConflict> {
    if snapshot.schema_version != CLIPBOARD_SUBGRAPH_SCHEMA_VERSION {
        return Err(invalid_clipboard(format!(
            "unsupported clipboard subgraph schema version {}",
            snapshot.schema_version
        )));
    }
    if snapshot.nodes.is_empty() {
        return Err(invalid_clipboard("clipboard subgraph contains no nodes"));
    }
    enforce_insert_limit("nodes", snapshot.nodes.len(), MAX_CLIPBOARD_NODES)?;
    enforce_insert_limit(
        "connections",
        snapshot.connections.len(),
        MAX_CLIPBOARD_CONNECTIONS,
    )?;
    enforce_insert_limit(
        "port bindings",
        snapshot.port_bindings.len(),
        MAX_CLIPBOARD_PORT_BINDINGS,
    )?;
    enforce_insert_limit(
        "input states",
        snapshot.input_states.len(),
        MAX_CLIPBOARD_INPUT_STATES,
    )?;

    let serialized = serde_json::to_vec(snapshot)
        .map_err(|error| invalid_clipboard(format!("clipboard serialization failed: {error}")))?;
    enforce_insert_limit(
        "serialized bytes",
        serialized.len(),
        MAX_CLIPBOARD_SERIALIZED_BYTES,
    )?;
    let mut parameter_bytes = 0usize;
    for state in &snapshot.input_states {
        if state
            .state
            .literal_override
            .as_ref()
            .is_some_and(|value| json_depth(value) > MAX_CLIPBOARD_VALUE_DEPTH)
        {
            return Err(invalid_clipboard(
                "clipboard input literal exceeds depth limit",
            ));
        }
    }
    for node in &snapshot.nodes {
        let bytes = serde_json::to_vec(&node.parameters).map_err(|error| {
            invalid_clipboard(format!("clipboard parameter serialization failed: {error}"))
        })?;
        parameter_bytes = parameter_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| invalid_clipboard("clipboard parameter size overflow"))?;
        for value in node.parameters.values() {
            if json_depth(value) > MAX_CLIPBOARD_VALUE_DEPTH {
                return Err(invalid_clipboard(
                    "clipboard parameter value exceeds depth limit",
                ));
            }
        }
    }
    enforce_insert_limit(
        "parameter bytes",
        parameter_bytes,
        MAX_CLIPBOARD_PARAMETER_BYTES,
    )
}

fn validate_insert_nodes(
    graph_path: &GraphResourcePath,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    snapshot: &ClipboardSubgraphDto,
    anchor: NodePosition,
) -> Result<BTreeMap<ClipboardNodeId, NodeTypeId>, MutationConflict> {
    if !anchor.x.is_finite() || !anchor.y.is_finite() {
        return Err(invalid_clipboard("subgraph anchor must be finite"));
    }
    let mut node_types = BTreeMap::new();
    for node in &snapshot.nodes {
        validate_local_identity("node", node.local_id.0.as_ref())?;
        let node_type = creation_node_type(&node.creation).clone();
        if node_types
            .insert(node.local_id.clone(), node_type.clone())
            .is_some()
        {
            return Err(invalid_clipboard(format!(
                "duplicate clipboard node ID '{}'",
                node.local_id.0
            )));
        }
        let position = NodePosition {
            x: anchor.x + node.relative_position.x,
            y: anchor.y + node.relative_position.y,
        };
        if !node.relative_position.x.is_finite()
            || !node.relative_position.y.is_finite()
            || !position.x.is_finite()
            || !position.y.is_finite()
        {
            return Err(invalid_clipboard(format!(
                "clipboard node '{}' has a non-finite target position",
                node.local_id.0
            )));
        }
        validate_node_creation(graph_path, registry, catalog, node)?;
    }
    Ok(node_types)
}

fn validate_node_creation(
    graph_path: &GraphResourcePath,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    node: &ClipboardNodeDto,
) -> Result<(), MutationConflict> {
    let node_type = creation_node_type(&node.creation);
    let protocol = registry.protocol(node_type).ok_or_else(|| {
        invalid_clipboard(format!("clipboard node type '{node_type}' is unavailable"))
    })?;
    if protocol.managed_role.is_some() {
        return Err(invalid_clipboard(format!(
            "clipboard node type '{node_type}' is managed"
        )));
    }
    validate_node_scope(graph_path, protocol)
        .map_err(|error| invalid_clipboard(error.to_string()))?;

    match &node.creation {
        ClipboardNodeCreationDto::Static { .. } => {
            if matches!(
                protocol.instance_display,
                NodeInstanceDisplaySpec::ResourceParameter { .. }
            ) || !matches!(
                authoritative_static_descriptor(registry, protocol),
                Some(NodeCreationDescriptor::Static { .. })
                    | Some(NodeCreationDescriptor::ParameterizedStatic { .. })
            ) {
                return Err(invalid_clipboard(format!(
                    "clipboard static identity does not match registry authority for '{node_type}'"
                )));
            }
        }
        ClipboardNodeCreationDto::ResourceBound {
            resource_path,
            create_args,
            ..
        } => validate_resource_creation(
            graph_path,
            protocol,
            resource_path,
            *create_args,
            catalog,
            &node.parameters,
        )?,
    }
    validate_parameters_with_registry(registry, protocol, &node.parameters)
        .map_err(|error| invalid_clipboard(error.to_string()))
}

fn validate_resource_creation(
    graph_path: &GraphResourcePath,
    protocol: &crate::node_system::protocol::NodeProtocol,
    resource_path: &CatalogResourcePath,
    create_args: ResourceBoundCreateArgsDto,
    catalog: &CatalogMutationValidationSnapshot,
    parameters: &ParameterValues,
) -> Result<(), MutationConflict> {
    let resource = catalog.resources.get(resource_path).ok_or_else(|| {
        unavailable_resource(format!(
            "referenced resource '{}' is unavailable",
            resource_path.as_str()
        ))
    })?;
    validate_resource_path(resource_path, create_args)?;
    let (allowed, binding, kind, in_scope) = match (create_args, resource) {
        (
            ResourceBoundCreateArgsDto::Function,
            CatalogMutationResource::Function {
                allowed_node_type_id,
                parameter_binding,
                ..
            },
        ) => (
            allowed_node_type_id == &protocol.type_id,
            parameter_binding.as_ref(),
            ResourceDisplayKind::Function,
            true,
        ),
        (
            ResourceBoundCreateArgsDto::Variable,
            CatalogMutationResource::Variable {
                allowed_node_type_ids,
                parameter_binding,
                scope,
                ..
            },
        ) => (
            allowed_node_type_ids.contains(&protocol.type_id),
            parameter_binding.as_ref(),
            ResourceDisplayKind::Variable,
            variable_in_scope(graph_path, scope),
        ),
        (
            ResourceBoundCreateArgsDto::Database,
            CatalogMutationResource::Database {
                allowed_node_type_id,
                parameter_binding,
                ..
            },
        ) => (
            allowed_node_type_id == &protocol.type_id,
            parameter_binding.as_ref(),
            ResourceDisplayKind::Database,
            true,
        ),
        _ => {
            return Err(invalid_clipboard(format!(
                "resource '{}' kind does not match clipboard creation arguments",
                resource_path.as_str()
            )));
        }
    };
    if !allowed || !in_scope {
        return Err(unavailable_resource(format!(
            "resource '{}' is unavailable for this graph and node type",
            resource_path.as_str()
        )));
    }
    let NodeInstanceDisplaySpec::ResourceParameter {
        parameter,
        kind: expected_kind,
    } = &protocol.instance_display
    else {
        return Err(invalid_clipboard(format!(
            "node type '{}' is not resource-bound",
            protocol.type_id
        )));
    };
    if parameter.as_str() != binding || *expected_kind != kind {
        return Err(invalid_clipboard(format!(
            "resource '{}' binding does not match protocol authority",
            resource_path.as_str()
        )));
    }
    if parameters.get(parameter)
        != Some(&serde_json::Value::String(
            resource_path.as_str().to_owned(),
        ))
    {
        return Err(invalid_clipboard(format!(
            "resource '{}' is not bound by the clipboard node parameters",
            resource_path.as_str()
        )));
    }
    Ok(())
}

fn validate_resource_path(
    resource_path: &CatalogResourcePath,
    create_args: ResourceBoundCreateArgsDto,
) -> Result<(), MutationConflict> {
    let path = resource_path.as_str();
    let valid = match create_args {
        ResourceBoundCreateArgsDto::Function => crate::project::GraphResourcePath::new(path)
            .is_ok_and(|canonical| {
                canonical.as_str() == path && canonical.as_str().starts_with("functions/")
            }),
        ResourceBoundCreateArgsDto::Variable => path
            .strip_prefix("variables/")
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
            .is_some_and(|id| format!("variables/{id}") == path),
        ResourceBoundCreateArgsDto::Database => path
            .strip_prefix("databases/")
            .is_some_and(|id| !id.is_empty()),
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_clipboard(format!(
            "resource path '{path}' is malformed for its creation arguments"
        )))
    }
}

fn validate_portable_references(
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    snapshot: &ClipboardSubgraphDto,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
) -> Result<BTreeSet<LocalInstanceKey>, MutationConflict> {
    let mut binding_addresses = BTreeSet::new();
    let mut instance_keys = BTreeSet::new();
    for entry in &snapshot.port_bindings {
        let ClipboardPortRefDto::Instance {
            local_instance_id, ..
        } = &entry.address.port
        else {
            return Err(invalid_clipboard(
                "clipboard port bindings require instance addresses",
            ));
        };
        validate_local_identity("port instance", local_instance_id.0.as_ref())?;
        if !binding_addresses.insert(entry.address.clone()) {
            return Err(invalid_clipboard(format!(
                "duplicate clipboard port binding at {:?}",
                entry.address
            )));
        }
        let spec = portable_port_spec(&entry.address, node_types, registry)?;
        let compatible = matches!(
            (&spec.instances, &entry.binding),
            (
                PortInstances::UserCreated { .. },
                ClipboardDynamicPortBindingDto::UserCreated { .. }
            ) | (
                PortInstances::Derived { .. },
                ClipboardDynamicPortBindingDto::Resolved { .. }
                    | ClipboardDynamicPortBindingDto::Orphan { .. }
            )
        );
        if !compatible {
            return Err(invalid_clipboard(format!(
                "clipboard binding kind does not match port template '{}'",
                spec.key
            )));
        }
        instance_keys.insert(local_instance_key(&entry.address, node_types, registry)?);
        let binding = DynamicPortBinding::from(entry.binding.clone());
        validate_dynamic_origin(&entry.address, &binding, snapshot, registry, catalog)?;
    }
    validate_instance_cardinality(snapshot, node_types, registry)?;

    let mut state_addresses = BTreeSet::new();
    for entry in &snapshot.input_states {
        if !state_addresses.insert(entry.address.clone()) {
            return Err(invalid_clipboard(format!(
                "duplicate clipboard input state at {:?}",
                entry.address
            )));
        }
        validate_endpoint(&entry.address, node_types, registry, &binding_addresses)?;
    }

    let mut connections = BTreeSet::new();
    for connection in &snapshot.connections {
        validate_endpoint(&connection.output, node_types, registry, &binding_addresses)?;
        validate_endpoint(&connection.input, node_types, registry, &binding_addresses)?;
        if !connections.insert((
            connection.output.clone(),
            connection.input.clone(),
            connection.order.clone(),
        )) {
            return Err(invalid_clipboard(
                "clipboard subgraph contains a duplicate connection",
            ));
        }
    }
    Ok(instance_keys)
}

fn portable_port_spec<'a>(
    address: &ClipboardPortAddressDto,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
    registry: &'a NodeRegistry,
) -> Result<&'a crate::node_system::protocol::PortSpec, MutationConflict> {
    let node_type = node_types.get(&address.node_id).ok_or_else(|| {
        invalid_clipboard(format!(
            "clipboard address references missing node '{}'",
            address.node_id.0
        ))
    })?;
    let protocol = registry
        .protocol(node_type)
        .ok_or_else(|| invalid_clipboard(format!("node type '{node_type}' is unavailable")))?;
    let key = match &address.port {
        ClipboardPortRefDto::Declared { key } => key,
        ClipboardPortRefDto::Instance { template, .. } => template,
    };
    protocol
        .interface
        .ports
        .iter()
        .find(|spec| &spec.key == key)
        .ok_or_else(|| {
            invalid_clipboard(format!(
                "clipboard address references unknown port '{key}' on node '{}'",
                address.node_id.0
            ))
        })
}

fn validate_endpoint(
    address: &ClipboardPortAddressDto,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
    registry: &NodeRegistry,
    binding_addresses: &BTreeSet<ClipboardPortAddressDto>,
) -> Result<(), MutationConflict> {
    let spec = portable_port_spec(address, node_types, registry)?;
    match &address.port {
        ClipboardPortRefDto::Declared { .. }
            if matches!(spec.instances, PortInstances::Declared) =>
        {
            Ok(())
        }
        ClipboardPortRefDto::Instance {
            local_instance_id, ..
        } if !local_instance_id.0.is_empty() && binding_addresses.contains(address) => Ok(()),
        ClipboardPortRefDto::Declared { .. } => Err(invalid_clipboard(format!(
            "port '{}' requires an instance address",
            spec.key
        ))),
        ClipboardPortRefDto::Instance { .. } => Err(invalid_clipboard(format!(
            "instance port '{}' has no clipboard binding",
            spec.key
        ))),
    }
}

fn validate_instance_cardinality(
    snapshot: &ClipboardSubgraphDto,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
    registry: &NodeRegistry,
) -> Result<(), MutationConflict> {
    for (local_node, node_type) in node_types {
        let protocol = registry
            .protocol(node_type)
            .expect("validated clipboard node protocols remain registered");
        for group in &protocol.interface.member_groups {
            let mut members = BTreeMap::<ClipboardPortInstanceId, BTreeSet<PortKey>>::new();
            for entry in snapshot
                .port_bindings
                .iter()
                .filter(|entry| &entry.address.node_id == local_node)
            {
                let ClipboardPortRefDto::Instance {
                    template,
                    local_instance_id,
                } = &entry.address.port
                else {
                    continue;
                };
                if group.templates.contains(template) {
                    members
                        .entry(local_instance_id.clone())
                        .or_default()
                        .insert(template.clone());
                }
            }
            let required = group.templates.iter().cloned().collect::<BTreeSet<_>>();
            if members.values().any(|templates| templates != &required)
                || members.len() < usize::from(group.min)
                || group
                    .max
                    .is_some_and(|maximum| members.len() > usize::from(maximum))
            {
                return Err(invalid_clipboard(format!(
                    "clipboard node '{}' has invalid grouped port cardinality",
                    local_node.0
                )));
            }
        }
        for spec in protocol.interface.ports.iter() {
            if protocol
                .interface
                .member_group_for_template(&spec.key)
                .is_some()
            {
                continue;
            }
            let PortInstances::UserCreated { min, max } = spec.instances else {
                continue;
            };
            let count = snapshot
                .port_bindings
                .iter()
                .filter_map(|entry| {
                    if &entry.address.node_id != local_node {
                        return None;
                    }
                    match &entry.address.port {
                        ClipboardPortRefDto::Instance {
                            template,
                            local_instance_id,
                        } if template == &spec.key => Some(local_instance_id),
                        _ => None,
                    }
                })
                .collect::<BTreeSet<_>>()
                .len();
            if count < usize::from(min) || max.is_some_and(|maximum| count > usize::from(maximum)) {
                return Err(invalid_clipboard(format!(
                    "clipboard node '{}' has invalid cardinality for port '{}'",
                    local_node.0, spec.key
                )));
            }
        }
    }
    Ok(())
}

fn validate_dynamic_origin(
    address: &ClipboardPortAddressDto,
    binding: &DynamicPortBinding,
    snapshot: &ClipboardSubgraphDto,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
) -> Result<(), MutationConflict> {
    let DynamicPortBinding::Resolved {
        origin, last_known, ..
    } = binding
    else {
        return Ok(());
    };
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.local_id == address.node_id)
        .expect("portable addresses reference validated nodes");
    let protocol = registry
        .protocol(creation_node_type(&node.creation))
        .expect("clipboard node protocols were validated");
    let template = match &address.port {
        ClipboardPortRefDto::Instance { template, .. } => template,
        ClipboardPortRefDto::Declared { .. } => {
            return Err(invalid_clipboard(
                "resolved dynamic binding requires an instance address",
            ));
        }
    };
    let spec = protocol
        .interface
        .ports
        .iter()
        .find(|spec| &spec.key == template)
        .expect("clipboard port templates were validated");
    let authoritative_type = validate_resolved_dynamic_binding_authority(
        protocol,
        spec,
        &node.parameters,
        origin,
        catalog,
    )
    .map_err(|error| match error {
        MutationConflict::ReferencedResourceUnavailable(message) => {
            MutationConflict::ReferencedResourceUnavailable(message)
        }
        other => invalid_clipboard(other.to_string()),
    })?;
    if last_known.value_type.as_ref() != Some(&authoritative_type) {
        return Err(invalid_clipboard(format!(
            "resolved dynamic binding for template '{}' has forged last-known type",
            spec.key
        )));
    }
    Ok(())
}

fn local_instance_key(
    address: &ClipboardPortAddressDto,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
    registry: &NodeRegistry,
) -> Result<LocalInstanceKey, MutationConflict> {
    let ClipboardPortRefDto::Instance {
        template,
        local_instance_id,
    } = &address.port
    else {
        return Err(invalid_clipboard("declared port has no local instance key"));
    };
    let node_type = &node_types[&address.node_id];
    let protocol = registry
        .protocol(node_type)
        .expect("validated clipboard node protocols remain registered");
    let scope = protocol
        .interface
        .member_group_for_template(template)
        .and_then(|group| group.templates.first())
        .unwrap_or(template)
        .clone();
    Ok(LocalInstanceKey {
        node_id: address.node_id.clone(),
        scope,
        local_instance_id: local_instance_id.clone(),
    })
}

fn plan_instantiation(
    document: &GraphDocument,
    registry: &NodeRegistry,
    snapshot: &ClipboardSubgraphDto,
    anchor: NodePosition,
    node_ids: &BTreeMap<ClipboardNodeId, NodeId>,
    instance_ids: &BTreeMap<LocalInstanceKey, PortInstanceId>,
    connection_ids: &[ConnectionId],
) -> Result<GraphDocumentPatch, MutationConflict> {
    let node_types = snapshot
        .nodes
        .iter()
        .map(|node| {
            (
                node.local_id.clone(),
                creation_node_type(&node.creation).clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut operations = snapshot
        .nodes
        .iter()
        .map(|node| GraphDocumentOperation::InsertNode {
            node: DocumentNode {
                id: node_ids[&node.local_id],
                node_type: creation_node_type(&node.creation).clone(),
                position: NodePosition {
                    x: anchor.x + node.relative_position.x,
                    y: anchor.y + node.relative_position.y,
                },
                parameters: node.parameters.clone(),
                user_label: node.user_label.clone(),
            },
        })
        .collect::<Vec<_>>();
    for entry in &snapshot.port_bindings {
        operations.push(GraphDocumentOperation::InsertPortBinding {
            address: instantiate_address(
                &entry.address,
                &node_types,
                registry,
                node_ids,
                instance_ids,
            )?,
            binding: entry.binding.clone().into(),
        });
    }

    let mut staged = document.clone();
    GraphDocumentPatch::new(operations.clone())
        .apply_without_revision(&mut staged)
        .map_err(|error| invalid_clipboard(format!("node and port staging failed: {error}")))?;
    for operation in &operations {
        if let GraphDocumentOperation::InsertPortBinding { address, .. } = operation {
            validate_subgraph_port(&staged, registry, address)
                .map_err(|error| invalid_clipboard(error.to_string()))?;
        }
    }

    for entry in &snapshot.input_states {
        let address = instantiate_address(
            &entry.address,
            &node_types,
            registry,
            node_ids,
            instance_ids,
        )?;
        validate_literal_target(
            &staged,
            registry,
            &address,
            entry.state.literal_override.as_ref(),
        )
        .map_err(|error| invalid_clipboard(error.to_string()))?;
        let operation = GraphDocumentOperation::SetInputState {
            before: staged.input_states.get(&address).cloned(),
            address,
            after: Some(entry.state.clone()),
        };
        GraphDocumentPatch::new(vec![operation.clone()])
            .apply_without_revision(&mut staged)
            .map_err(|error| invalid_clipboard(format!("input state staging failed: {error}")))?;
        operations.push(operation);
    }

    for (index, entry) in snapshot.connections.iter().enumerate() {
        let output =
            instantiate_address(&entry.output, &node_types, registry, node_ids, instance_ids)?;
        let input =
            instantiate_address(&entry.input, &node_types, registry, node_ids, instance_ids)?;
        validate_subgraph_connection(&staged, registry, &output, &input, entry.order.as_ref())
            .map_err(|error| invalid_clipboard(error.to_string()))?;
        let operation = GraphDocumentOperation::InsertConnection {
            connection: DocumentConnection {
                id: connection_ids[index],
                output,
                input,
                order: entry.order.clone(),
            },
        };
        GraphDocumentPatch::new(vec![operation.clone()])
            .apply_without_revision(&mut staged)
            .map_err(|error| invalid_clipboard(format!("connection staging failed: {error}")))?;
        operations.push(operation);
    }
    Ok(GraphDocumentPatch::new(operations))
}

fn instantiate_address(
    address: &ClipboardPortAddressDto,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
    registry: &NodeRegistry,
    node_ids: &BTreeMap<ClipboardNodeId, NodeId>,
    instance_ids: &BTreeMap<LocalInstanceKey, PortInstanceId>,
) -> Result<PortAddress, MutationConflict> {
    let node_id = node_ids[&address.node_id];
    let port = match &address.port {
        ClipboardPortRefDto::Declared { key } => PortRef::Declared { key: key.clone() },
        ClipboardPortRefDto::Instance { template, .. } => PortRef::Instance {
            template: template.clone(),
            instance_id: instance_ids[&local_instance_key(address, node_types, registry)?],
        },
    };
    Ok(PortAddress { node_id, port })
}

fn temporary_node_ids<'a>(
    document: &GraphDocument,
    local_ids: impl Iterator<Item = &'a ClipboardNodeId>,
) -> BTreeMap<ClipboardNodeId, NodeId> {
    let mut used = document.nodes.keys().copied().collect::<BTreeSet<_>>();
    local_ids
        .enumerate()
        .map(|(index, local_id)| {
            let mut value = u128::MAX - index as u128;
            let id = loop {
                let candidate = NodeId::from_uuid(uuid::Uuid::from_u128(value));
                if used.insert(candidate) {
                    break candidate;
                }
                value -= 1;
            };
            (local_id.clone(), id)
        })
        .collect()
}

fn temporary_port_instance_ids<'a>(
    document: &GraphDocument,
    keys: impl Iterator<Item = &'a LocalInstanceKey>,
) -> BTreeMap<LocalInstanceKey, PortInstanceId> {
    let mut used = document
        .port_bindings
        .keys()
        .filter_map(|address| match address.port {
            PortRef::Instance { instance_id, .. } => Some(instance_id),
            PortRef::Declared { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    keys.enumerate()
        .map(|(index, key)| {
            let mut value = u128::MAX / 2 - index as u128;
            let id = loop {
                let candidate = PortInstanceId::from_uuid(uuid::Uuid::from_u128(value));
                if used.insert(candidate) {
                    break candidate;
                }
                value -= 1;
            };
            (key.clone(), id)
        })
        .collect()
}

fn temporary_connection_ids(document: &GraphDocument, count: usize) -> Vec<ConnectionId> {
    let mut used = document
        .connections
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    (0..count)
        .map(|index| {
            let mut value = u128::MAX / 4 - index as u128;
            loop {
                let candidate = ConnectionId::from_uuid(uuid::Uuid::from_u128(value));
                if used.insert(candidate) {
                    break candidate;
                }
                value -= 1;
            }
        })
        .collect()
}

fn fresh_node_ids<'a>(
    document: &GraphDocument,
    local_ids: impl Iterator<Item = &'a ClipboardNodeId>,
) -> BTreeMap<ClipboardNodeId, NodeId> {
    let mut used = document.nodes.keys().copied().collect::<BTreeSet<_>>();
    local_ids
        .map(|local_id| {
            let id = loop {
                let candidate = NodeId::new();
                if used.insert(candidate) {
                    break candidate;
                }
            };
            (local_id.clone(), id)
        })
        .collect()
}

fn fresh_port_instance_ids<'a>(
    document: &GraphDocument,
    keys: impl Iterator<Item = &'a LocalInstanceKey>,
) -> BTreeMap<LocalInstanceKey, PortInstanceId> {
    let mut used = document
        .port_bindings
        .keys()
        .filter_map(|address| match address.port {
            PortRef::Instance { instance_id, .. } => Some(instance_id),
            PortRef::Declared { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    keys.map(|key| {
        let id = loop {
            let candidate = PortInstanceId::new();
            if used.insert(candidate) {
                break candidate;
            }
        };
        (key.clone(), id)
    })
    .collect()
}

fn fresh_connection_ids(document: &GraphDocument, count: usize) -> Vec<ConnectionId> {
    let mut used = document
        .connections
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    (0..count)
        .map(|_| {
            loop {
                let candidate = ConnectionId::new();
                if used.insert(candidate) {
                    break candidate;
                }
            }
        })
        .collect()
}

fn creation_node_type(creation: &ClipboardNodeCreationDto) -> &NodeTypeId {
    match creation {
        ClipboardNodeCreationDto::Static { node_type_id }
        | ClipboardNodeCreationDto::ResourceBound { node_type_id, .. } => node_type_id,
    }
}

fn validate_local_identity(kind: &str, value: &str) -> Result<(), MutationConflict> {
    if value.is_empty() {
        Err(invalid_clipboard(format!(
            "clipboard {kind} identity must not be empty"
        )))
    } else {
        Ok(())
    }
}

fn enforce_insert_limit(name: &str, actual: usize, limit: usize) -> Result<(), MutationConflict> {
    if actual > limit {
        Err(invalid_clipboard(format!(
            "clipboard subgraph {name} limit exceeded ({actual} > {limit})"
        )))
    } else {
        Ok(())
    }
}

fn invalid_clipboard(message: impl Into<Box<str>>) -> MutationConflict {
    MutationConflict::ClipboardSubgraphInvalid(message.into())
}

fn unavailable_resource(message: impl Into<Box<str>>) -> MutationConflict {
    MutationConflict::ReferencedResourceUnavailable(message.into())
}

fn validate_targets(
    document: &GraphDocument,
    node_ids: Vec<NodeId>,
) -> Result<BTreeSet<NodeId>, MutationConflict> {
    if node_ids.is_empty() {
        return Err(invalid_export("subgraph export requires at least one node"));
    }
    let selected = node_ids.iter().copied().collect::<BTreeSet<_>>();
    if selected.len() != node_ids.len() {
        return Err(invalid_export(
            "subgraph export contains a duplicate direct target",
        ));
    }
    if let Some(missing) = selected
        .iter()
        .find(|node_id| !document.nodes.contains_key(node_id))
    {
        return Err(invalid_export(format!(
            "subgraph export node '{missing}' does not exist"
        )));
    }
    Ok(selected)
}

fn authoritative_creation(
    graph_path: &GraphResourcePath,
    node: &super::DocumentNode,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
) -> Result<ClipboardNodeCreationDto, MutationConflict> {
    let protocol = registry.protocol(&node.node_type).ok_or_else(|| {
        invalid_export(format!(
            "subgraph export references unknown node type '{}'",
            node.node_type
        ))
    })?;
    if protocol.managed_role.is_some() {
        return Err(invalid_export(format!(
            "managed node '{}' cannot be exported",
            node.id
        )));
    }

    let matches = matching_resources(graph_path, node, catalog)?;
    if matches.len() > 1 {
        return Err(invalid_export(format!(
            "node '{}' matches multiple authoritative resources",
            node.id
        )));
    }
    if let Some((resource_path, create_args)) = matches.into_iter().next() {
        return Ok(ClipboardNodeCreationDto::ResourceBound {
            node_type_id: node.node_type.clone(),
            resource_path,
            create_args,
        });
    }
    if matches!(
        protocol.instance_display,
        NodeInstanceDisplaySpec::ResourceParameter { .. }
    ) {
        return Err(invalid_export(format!(
            "node '{}' has no authoritative catalog resource",
            node.id
        )));
    }
    match authoritative_static_descriptor(registry, protocol) {
        Some(NodeCreationDescriptor::Static { .. })
        | Some(NodeCreationDescriptor::ParameterizedStatic { .. }) => {
            Ok(ClipboardNodeCreationDto::Static {
                node_type_id: node.node_type.clone(),
            })
        }
        _ => Err(invalid_export(format!(
            "node '{}' has no authoritative creation identity",
            node.id
        ))),
    }
}

fn matching_resources(
    graph_path: &GraphResourcePath,
    node: &super::DocumentNode,
    catalog: &CatalogMutationValidationSnapshot,
) -> Result<Vec<(CatalogResourcePath, ResourceBoundCreateArgsDto)>, MutationConflict> {
    let mut matches = Vec::new();
    for (resource_path, resource) in &catalog.resources {
        let (allowed, parameter_binding, create_args, in_scope) = match resource {
            CatalogMutationResource::Function {
                allowed_node_type_id,
                parameter_binding,
                ..
            } => (
                allowed_node_type_id == &node.node_type,
                parameter_binding.as_ref(),
                ResourceBoundCreateArgsDto::Function,
                true,
            ),
            CatalogMutationResource::Variable {
                allowed_node_type_ids,
                parameter_binding,
                scope,
                ..
            } => (
                allowed_node_type_ids.contains(&node.node_type),
                parameter_binding.as_ref(),
                ResourceBoundCreateArgsDto::Variable,
                variable_in_scope(graph_path, scope),
            ),
            CatalogMutationResource::Database {
                allowed_node_type_id,
                parameter_binding,
                ..
            } => (
                allowed_node_type_id == &node.node_type,
                parameter_binding.as_ref(),
                ResourceBoundCreateArgsDto::Database,
                true,
            ),
        };
        if !allowed || !in_scope {
            continue;
        }
        let key = ParameterKey::new(parameter_binding).map_err(|error| {
            invalid_export(format!(
                "catalog resource '{}' has an invalid parameter binding: {error}",
                resource_path.as_str()
            ))
        })?;
        if node.parameters.get(&key)
            == Some(&serde_json::Value::String(
                resource_path.as_str().to_owned(),
            ))
        {
            matches.push((resource_path.clone(), create_args));
        }
    }
    Ok(matches)
}

fn variable_in_scope(
    graph_path: &GraphResourcePath,
    scope: &crate::variable::VariableScope,
) -> bool {
    match scope {
        crate::variable::VariableScope::Global => true,
        crate::variable::VariableScope::Event { event_path } => {
            event_path.as_str() == graph_path.0.as_ref()
        }
        crate::variable::VariableScope::Function { function_path } => {
            function_path.as_str() == graph_path.0.as_ref()
        }
    }
}

fn local_instance_ids<'a>(
    bindings: impl Iterator<Item = &'a PortAddress>,
    states: impl Iterator<Item = &'a PortAddress>,
    connections: impl Iterator<Item = &'a PortAddress>,
) -> BTreeMap<PortInstanceId, ClipboardPortInstanceId> {
    let addresses = bindings
        .chain(states)
        .chain(connections)
        .collect::<BTreeSet<_>>();
    let mut instances = BTreeMap::new();
    for address in addresses {
        if let PortRef::Instance { instance_id, .. } = address.port {
            let next = instances.len();
            instances
                .entry(instance_id)
                .or_insert_with(|| ClipboardPortInstanceId(format!("port/{next}").into()));
        }
    }
    instances
}

fn rewrite_address(
    address: &PortAddress,
    nodes: &BTreeMap<NodeId, ClipboardNodeId>,
    instances: &BTreeMap<PortInstanceId, ClipboardPortInstanceId>,
) -> Result<ClipboardPortAddressDto, MutationConflict> {
    let node_id = nodes
        .get(&address.node_id)
        .cloned()
        .ok_or_else(|| invalid_export("subgraph address references an unselected node"))?;
    let port = match &address.port {
        PortRef::Declared { key } => ClipboardPortRefDto::Declared { key: key.clone() },
        PortRef::Instance {
            template,
            instance_id,
        } => ClipboardPortRefDto::Instance {
            template: template.clone(),
            local_instance_id: instances
                .get(instance_id)
                .cloned()
                .ok_or_else(|| invalid_export("subgraph address has no local port identity"))?,
        },
    };
    Ok(ClipboardPortAddressDto { node_id, port })
}

fn validate_parameter_values(
    parameters: &ParameterValues,
    total_bytes: &mut usize,
) -> Result<(), MutationConflict> {
    for value in parameters.values() {
        if json_depth(value) > MAX_CLIPBOARD_VALUE_DEPTH {
            return Err(invalid_export(
                "subgraph parameter value exceeds depth limit",
            ));
        }
    }
    let bytes = serde_json::to_vec(parameters)
        .map_err(|error| invalid_export(format!("parameter serialization failed: {error}")))?
        .len();
    *total_bytes = total_bytes
        .checked_add(bytes)
        .ok_or_else(|| invalid_export("subgraph parameter size overflow"))?;
    enforce_limit(
        "parameter bytes",
        *total_bytes,
        MAX_CLIPBOARD_PARAMETER_BYTES,
    )
}

fn json_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Array(values) => {
            1 + values.iter().map(json_depth).max().unwrap_or_default()
        }
        serde_json::Value::Object(values) => {
            1 + values.values().map(json_depth).max().unwrap_or_default()
        }
        _ => 1,
    }
}

fn enforce_limit(name: &str, actual: usize, limit: usize) -> Result<(), MutationConflict> {
    if actual > limit {
        Err(invalid_export(format!(
            "subgraph export {name} limit exceeded ({actual} > {limit})"
        )))
    } else {
        Ok(())
    }
}

fn invalid_export(message: impl Into<Box<str>>) -> MutationConflict {
    MutationConflict::InvalidEditorMutation(message.into())
}
