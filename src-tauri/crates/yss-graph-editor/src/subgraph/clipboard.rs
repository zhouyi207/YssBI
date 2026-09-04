use super::*;

pub(crate) const CLIPBOARD_SUBGRAPH_SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_CLIPBOARD_NODES: usize = 500;
pub(crate) const MAX_CLIPBOARD_CONNECTIONS: usize = 2_000;
pub(crate) const MAX_CLIPBOARD_PORT_BINDINGS: usize = 4_000;
pub(crate) const MAX_CLIPBOARD_INPUT_STATES: usize = 4_000;
pub(crate) const MAX_CLIPBOARD_PARAMETER_BYTES: usize = 1_048_576;
pub(crate) const MAX_CLIPBOARD_VALUE_DEPTH: usize = 64;
pub(crate) const MAX_CLIPBOARD_SERIALIZED_BYTES: usize = 4_194_304;

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
pub enum ClipboardNodeCreation {
    Static {
        node_type_id: NodeTypeId,
    },
    ResourceBound {
        node_type_id: NodeTypeId,
        resource_path: CatalogResourcePath,
        create_args: ResourceBoundCreateArgs,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClipboardPortRef {
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
pub struct ClipboardPortAddress {
    pub node_id: ClipboardNodeId,
    pub port: ClipboardPortRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardNode {
    pub local_id: ClipboardNodeId,
    pub creation: ClipboardNodeCreation,
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
pub enum ClipboardDynamicMemberOrigin {
    FunctionParameter {
        function: GraphResourcePath,
        parameter: FunctionParameterId,
    },
    SchemaField {
        source: SchemaSourceIdentity,
        field: SchemaFieldIdentity,
    },
}

impl From<&DynamicMemberLocator> for ClipboardDynamicMemberOrigin {
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

impl From<ClipboardDynamicMemberOrigin> for DynamicMemberLocator {
    fn from(origin: ClipboardDynamicMemberOrigin) -> Self {
        match origin {
            ClipboardDynamicMemberOrigin::FunctionParameter {
                function,
                parameter,
            } => Self::FunctionParameter {
                function,
                parameter,
            },
            ClipboardDynamicMemberOrigin::SchemaField { source, field } => {
                Self::SchemaField { source, field }
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardLastKnownPortMetadata {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<TypeExpr>,
}

impl From<&LastKnownPortMetadata> for ClipboardLastKnownPortMetadata {
    fn from(last_known: &LastKnownPortMetadata) -> Self {
        Self {
            label: last_known.label.clone(),
            value_type: last_known.value_type.clone(),
        }
    }
}

impl From<ClipboardLastKnownPortMetadata> for LastKnownPortMetadata {
    fn from(last_known: ClipboardLastKnownPortMetadata) -> Self {
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
pub enum ClipboardDynamicPortBinding {
    UserCreated {
        order: OrderKey,
    },
    Resolved {
        origin: ClipboardDynamicMemberOrigin,
        order: OrderKey,
        last_known: ClipboardLastKnownPortMetadata,
    },
    Orphan {
        origin: ClipboardDynamicMemberOrigin,
        order: OrderKey,
        last_known: ClipboardLastKnownPortMetadata,
    },
}

impl From<&DynamicPortBinding> for ClipboardDynamicPortBinding {
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

impl From<ClipboardDynamicPortBinding> for DynamicPortBinding {
    fn from(binding: ClipboardDynamicPortBinding) -> Self {
        match binding {
            ClipboardDynamicPortBinding::UserCreated { order } => Self::UserCreated { order },
            ClipboardDynamicPortBinding::Resolved {
                origin,
                order,
                last_known,
            } => Self::Resolved {
                origin: origin.into(),
                order,
                last_known: last_known.into(),
            },
            ClipboardDynamicPortBinding::Orphan {
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
pub struct ClipboardPortBinding {
    pub address: ClipboardPortAddress,
    pub binding: ClipboardDynamicPortBinding,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardInputState {
    pub address: ClipboardPortAddress,
    #[serde(
        serialize_with = "serialize_clipboard_input_state",
        deserialize_with = "deserialize_bounded_input_state"
    )]
    pub state: InputState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardConnection {
    pub output: ClipboardPortAddress,
    pub input: ClipboardPortAddress,
    pub order: Option<OrderKey>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardSubgraph {
    pub schema_version: u32,
    pub nodes: Vec<ClipboardNode>,
    pub port_bindings: Vec<ClipboardPortBinding>,
    pub input_states: Vec<ClipboardInputState>,
    pub connections: Vec<ClipboardConnection>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClipboardSubgraphWire {
    schema_version: u32,
    #[serde(deserialize_with = "deserialize_nodes")]
    nodes: Vec<ClipboardNode>,
    #[serde(deserialize_with = "deserialize_port_bindings")]
    port_bindings: Vec<ClipboardPortBinding>,
    #[serde(deserialize_with = "deserialize_input_states")]
    input_states: Vec<ClipboardInputState>,
    #[serde(deserialize_with = "deserialize_connections")]
    connections: Vec<ClipboardConnection>,
}

impl From<ClipboardSubgraphWire> for ClipboardSubgraph {
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
pub(crate) struct ValidatedClipboardSubgraph(pub(crate) ClipboardSubgraph);

/// Decodes untrusted clipboard JSON through the editor's byte and shape limits.
pub fn deserialize_clipboard_subgraph(bytes: &[u8]) -> Result<ClipboardSubgraph, MutationConflict> {
    if bytes.len() > MAX_CLIPBOARD_SERIALIZED_BYTES {
        return Err(invalid_clipboard(format!(
            "clipboard payload byte limit exceeded ({} > {})",
            bytes.len(),
            MAX_CLIPBOARD_SERIALIZED_BYTES
        )));
    }
    serde_json::from_slice::<ClipboardSubgraphWire>(bytes)
        .map(Into::into)
        .map_err(|error| invalid_clipboard(format!("clipboard payload is invalid: {error}")))
}

fn deserialize_nodes<'de, D>(deserializer: D) -> Result<Vec<ClipboardNode>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec(deserializer, MAX_CLIPBOARD_NODES, "clipboard nodes")
}

fn deserialize_port_bindings<'de, D>(deserializer: D) -> Result<Vec<ClipboardPortBinding>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec(
        deserializer,
        MAX_CLIPBOARD_PORT_BINDINGS,
        "clipboard port bindings",
    )
}

fn deserialize_input_states<'de, D>(deserializer: D) -> Result<Vec<ClipboardInputState>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec(
        deserializer,
        MAX_CLIPBOARD_INPUT_STATES,
        "clipboard input states",
    )
}

fn deserialize_connections<'de, D>(deserializer: D) -> Result<Vec<ClipboardConnection>, D::Error>
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
        literal_override: &'a Option<yss_graph_protocol::TypedValue>,
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
        literal_override: wire
            .literal_override
            .map(|value| serde_json::from_value(value).map_err(serde::de::Error::custom))
            .transpose()?,
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
