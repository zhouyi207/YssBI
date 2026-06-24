/// Pin 实例（运行时）
///
/// Pin 实例由 Graph 管理，不属于 Node。
/// Pin 不存储连接信息，所有连接由 ConnectionManager 管理。
use super::{PinDefinition, PinDirection, PinId, PinKind, PinOrder, PinRole};
use crate::graph::node::DataSchema;
use crate::graph::{DataValue, NodeId, TypeVarId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone)]
pub struct PinInstance {
    pub id: PinId,
    pub node_id: NodeId,
    pub definition: PinDefinition,
    pub order: PinOrder,
    pub type_var_id: Option<TypeVarId>,
    pub user_value: Option<DataValue>,
    /// 连接时传播的 schema（运行时缓存，不持久化）
    pub resolved_schema: Option<DataSchema>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PinInstanceSer<'a> {
    id: PinId,
    node_id: NodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    definition: Option<&'a PinDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pin_contract: Option<PinContractSer<'a>>,
    order: PinOrder,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_var_id: Option<TypeVarId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_value: Option<&'a DataValue>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PinContractSer<'a> {
    name: &'a str,
    direction: PinDirection,
    kind: PinKind,
    role: &'a PinRole,
    optional: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinInstanceDe {
    id: PinId,
    node_id: NodeId,
    definition: Option<PinDefinition>,
    pin_contract: Option<PinContractDe>,
    order: PinOrder,
    #[serde(default)]
    type_var_id: Option<TypeVarId>,
    #[serde(default)]
    user_value: Option<DataValue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinContractDe {
    name: String,
    direction: PinDirection,
    kind: PinKind,
    role: PinRole,
    #[serde(default)]
    optional: bool,
}

impl Serialize for PinInstance {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let (definition, pin_contract) = if self.definition.should_persist_full_definition() {
            (Some(&self.definition), None)
        } else {
            (
                None,
                Some(PinContractSer {
                    name: &self.definition.name,
                    direction: self.definition.direction,
                    kind: self.definition.kind,
                    role: &self.definition.role,
                    optional: self.definition.optional,
                }),
            )
        };

        PinInstanceSer {
            id: self.id,
            node_id: self.node_id,
            definition,
            pin_contract,
            order: self.order,
            type_var_id: self.type_var_id,
            user_value: self.user_value.as_ref(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PinInstance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = PinInstanceDe::deserialize(deserializer)?;
        let definition = raw.definition.or_else(|| {
            raw.pin_contract.map(|contract| PinDefinition {
                name: contract.name,
                direction: contract.direction,
                kind: contract.kind,
                role: contract.role,
                data_type: None,
                optional: contract.optional,
                default_value: None,
                meta_data: Default::default(),
            })
        }).ok_or_else(|| serde::de::Error::custom("pin requires definition or pinContract"))?;

        Ok(Self {
            id: raw.id,
            node_id: raw.node_id,
            definition,
            order: raw.order,
            type_var_id: raw.type_var_id,
            user_value: raw.user_value,
            resolved_schema: None,
        })
    }
}

impl PinInstance {
    /// 从定义创建实例
    pub fn from_definition(def: &PinDefinition, node_id: NodeId, order: i32) -> Self {
        Self {
            id: PinId::new(),
            node_id,
            definition: def.clone(),
            order: PinOrder(order),
            type_var_id: None,
            user_value: None,
            resolved_schema: None,
        }
    }

    pub fn with_type_var_id(mut self, type_var_id: Option<TypeVarId>) -> Self {
        self.type_var_id = type_var_id;
        self
    }

    pub fn is_data(&self) -> bool {
        matches!(self.definition.kind, PinKind::Data)
    }

    pub fn is_exec(&self) -> bool {
        matches!(self.definition.kind, PinKind::Exec)
    }

    pub fn is_input(&self) -> bool {
        matches!(self.definition.direction, PinDirection::Input)
    }

    pub fn is_output(&self) -> bool {
        matches!(self.definition.direction, PinDirection::Output)
    }
}
