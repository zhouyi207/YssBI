use super::{ConnectionId, GraphRevision, NodeId, PortInstanceId};
use crate::node_system::protocol::{NodeTypeId, ParameterKey, PortKey};
use serde::{Deserialize, Serialize};
pub type TypedValue = serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

pub type ParameterValues = BTreeMap<ParameterKey, TypedValue>;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentNode {
    pub id: NodeId,
    pub node_type: NodeTypeId,
    pub position: NodePosition,
    pub parameters: ParameterValues,
    pub user_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PortAddress {
    pub node_id: NodeId,
    pub port: PortRef,
}

impl PortAddress {
    pub fn declared(node_id: NodeId, key: PortKey) -> Self {
        Self {
            node_id,
            port: PortRef::Declared { key },
        }
    }

    pub fn instance(node_id: NodeId, template: PortKey, instance_id: PortInstanceId) -> Self {
        Self {
            node_id,
            port: PortRef::Instance {
                template,
                instance_id,
            },
        }
    }

    pub fn is_instance(&self) -> bool {
        matches!(self.port, PortRef::Instance { .. })
    }
}

impl fmt::Display for PortAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.port {
            PortRef::Declared { key } => write!(formatter, "{}:{key}", self.node_id),
            PortRef::Instance {
                template,
                instance_id,
            } => write!(formatter, "{}:{template}:{instance_id}", self.node_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PortRef {
    Declared {
        key: PortKey,
    },
    Instance {
        template: PortKey,
        instance_id: PortInstanceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrderKey(pub Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentConnection {
    pub id: ConnectionId,
    pub output: PortAddress,
    pub input: PortAddress,
    pub order: Option<OrderKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DynamicPortBinding {
    UserCreated {
        order: OrderKey,
    },
    Resolved {
        origin: DynamicMemberLocator,
        order: OrderKey,
    },
    Orphan {
        origin: DynamicMemberLocator,
        order: OrderKey,
        last_known: LastKnownPortMetadata,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DynamicMemberLocator {
    FunctionParameter {
        function: GraphResourcePath,
        parameter: FunctionParameterId,
    },
    SchemaField {
        source: SchemaSourceIdentity,
        field: SchemaFieldIdentity,
    },
}

macro_rules! string_identity {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Box<str>);
    };
}

string_identity!(GraphResourcePath);
string_identity!(FunctionParameterId);
string_identity!(SchemaSourceIdentity);
string_identity!(SchemaFieldIdentity);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastKnownPortMetadata {
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputState {
    pub literal_override: Option<TypedValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectiveInputBinding {
    Connections(Vec<ConnectionId>),
    Literal(TypedValue),
    ProtocolDefault(TypedValue),
    Unbound,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphDocument {
    #[serde(skip)]
    pub revision: GraphRevision,
    pub nodes: BTreeMap<NodeId, DocumentNode>,
    #[serde(with = "port_address_map")]
    pub port_bindings: BTreeMap<PortAddress, DynamicPortBinding>,
    pub connections: BTreeMap<ConnectionId, DocumentConnection>,
    #[serde(with = "port_address_map")]
    pub input_states: BTreeMap<PortAddress, InputState>,
}

mod port_address_map {
    use super::PortAddress;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S, V>(
        values: &BTreeMap<PortAddress, V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        V: Serialize,
    {
        values.iter().collect::<Vec<_>>().serialize(serializer)
    }

    pub fn deserialize<'de, D, V>(deserializer: D) -> Result<BTreeMap<PortAddress, V>, D::Error>
    where
        D: Deserializer<'de>,
        V: Deserialize<'de>,
    {
        let entries = Vec::<(PortAddress, V)>::deserialize(deserializer)?;
        let mut values = BTreeMap::new();
        for (address, value) in entries {
            if values.insert(address, value).is_some() {
                return Err(serde::de::Error::custom("duplicate port address"));
            }
        }
        Ok(values)
    }
}

impl Default for GraphDocument {
    fn default() -> Self {
        Self {
            revision: GraphRevision::INITIAL,
            nodes: BTreeMap::new(),
            port_bindings: BTreeMap::new(),
            connections: BTreeMap::new(),
            input_states: BTreeMap::new(),
        }
    }
}
