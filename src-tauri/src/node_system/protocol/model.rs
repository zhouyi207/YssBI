use super::{
    I18nKey, IconId, InterfaceResolverId, NodeCategoryId, NodeStyleId, NodeTypeId, ParameterSchema,
    PortKey, SchemaExpr, TypeConstraint, TypeExpr, TypeParameterId, TypedValue,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeProtocol {
    pub type_id: NodeTypeId,
    pub catalog: NodeCatalogProtocol,
    pub interface: NodeInterfaceProtocol,
    pub parameters: ParameterSchema,
    pub execution: ExecutionSemantics,
    pub scope: NodeScope,
    pub managed_role: Option<ManagedNodeRole>,
}

impl NodeProtocol {
    pub fn from_static(protocol: &'static StaticNodeProtocol) -> Result<Self, ProtocolError> {
        let type_id = parse_id(NodeTypeId::new(protocol.type_id))?;
        let catalog = NodeCatalogProtocol::from_static(protocol.catalog)?;
        let mut keys = BTreeSet::new();
        let mut ports = Vec::with_capacity(protocol.ports.len());

        for port in protocol.ports {
            let key = parse_id(PortKey::new(port.key))?;
            if !keys.insert(key.clone()) {
                return Err(ProtocolError::DuplicatePortKey(key));
            }
            let spec = PortSpec {
                key,
                label_key: parse_i18n_key(port.label_key)?,
                direction: port.direction,
                kind: port.kind,
                value_type: TypeExpr::Unknown,
                instances: port.instances.clone(),
                connections: port.connections,
                input_binding: port.input_binding.clone(),
                consumption: None,
                production: None,
                editor: PortEditorSpec::Default,
                schema: None,
            };
            validate_port_contract(&spec)?;
            ports.push(spec);
        }

        validate_execution(protocol.execution)?;
        Ok(Self {
            type_id,
            catalog,
            interface: NodeInterfaceProtocol::new(ports, Vec::new(), Vec::new())?,
            parameters: ParameterSchema::default(),
            execution: protocol.execution,
            scope: protocol.scope,
            managed_role: protocol.managed_role,
        })
    }
}

/// Source-level compatibility form. Registry startup interns this into the
/// complete owned protocol and validates all contracts.
#[derive(Debug, Clone, Copy)]
pub struct StaticNodeProtocol {
    pub type_id: &'static str,
    pub catalog: StaticNodeCatalogProtocol,
    pub ports: &'static [StaticPortSpec],
    pub execution: ExecutionSemantics,
    pub scope: NodeScope,
    pub managed_role: Option<ManagedNodeRole>,
}

#[derive(Debug, Clone, Copy)]
pub struct StaticNodeCatalogProtocol {
    pub title_key: &'static str,
    pub description_key: Option<&'static str>,
    pub documentation_key: Option<&'static str>,
    pub aliases_key: Option<&'static str>,
    pub category_id: &'static str,
    pub icon_id: &'static str,
    pub style_id: &'static str,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCatalogProtocol {
    pub title_key: I18nKey,
    pub description_key: Option<I18nKey>,
    pub documentation_key: Option<I18nKey>,
    pub aliases_key: Option<I18nKey>,
    pub category_id: NodeCategoryId,
    pub icon_id: IconId,
    pub style_id: NodeStyleId,
    pub hidden: bool,
}

impl NodeCatalogProtocol {
    fn from_static(catalog: StaticNodeCatalogProtocol) -> Result<Self, ProtocolError> {
        Ok(Self {
            title_key: parse_i18n_key(catalog.title_key)?,
            description_key: catalog.description_key.map(parse_i18n_key).transpose()?,
            documentation_key: catalog.documentation_key.map(parse_i18n_key).transpose()?,
            aliases_key: catalog.aliases_key.map(parse_i18n_key).transpose()?,
            category_id: parse_id(NodeCategoryId::new(catalog.category_id))?,
            icon_id: parse_id(IconId::new(catalog.icon_id))?,
            style_id: parse_id(NodeStyleId::new(catalog.style_id))?,
            hidden: catalog.hidden,
        })
    }
}

#[derive(Debug, Clone)]
pub struct StaticPortSpec {
    pub key: &'static str,
    pub label_key: &'static str,
    pub direction: PortDirection,
    pub kind: PortKind,
    pub instances: PortInstances,
    pub connections: ConnectionsPerPort,
    pub input_binding: Option<InputBindingSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeInterfaceProtocol {
    pub ports: Box<[PortSpec]>,
    pub type_parameters: Box<[TypeParameterId]>,
    pub type_constraints: Box<[TypeConstraint]>,
}

impl NodeInterfaceProtocol {
    pub fn new(
        ports: Vec<PortSpec>,
        type_parameters: Vec<TypeParameterId>,
        type_constraints: Vec<TypeConstraint>,
    ) -> Result<Self, ProtocolError> {
        let mut port_keys = BTreeSet::new();
        for port in &ports {
            if !port_keys.insert(port.key.clone()) {
                return Err(ProtocolError::DuplicatePortKey(port.key.clone()));
            }
            validate_port_contract(port)?;
        }
        let mut parameter_ids = BTreeSet::new();
        for id in &type_parameters {
            if !parameter_ids.insert(id.clone()) {
                return Err(ProtocolError::DuplicateTypeParameter(id.clone()));
            }
        }
        Ok(Self {
            ports: ports.into_boxed_slice(),
            type_parameters: type_parameters.into_boxed_slice(),
            type_constraints: type_constraints.into_boxed_slice(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortSpec {
    pub key: PortKey,
    pub label_key: I18nKey,
    pub direction: PortDirection,
    pub kind: PortKind,
    pub value_type: TypeExpr,
    pub instances: PortInstances,
    pub connections: ConnectionsPerPort,
    pub input_binding: Option<InputBindingSpec>,
    pub consumption: Option<InputConsumption>,
    pub production: Option<OutputProduction>,
    pub editor: PortEditorSpec,
    pub schema: Option<SchemaExpr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortKind {
    Data,
    Control,
    Effect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortInstances {
    Declared,
    UserCreated { min: u16, max: Option<u16> },
    Derived { resolver: InterfaceResolverId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionsPerPort {
    Single,
    Multiple { max: Option<u16>, ordered: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputBindingSpec {
    pub literal_policy: LiteralPolicy,
    pub default_value: Option<TypedValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiteralPolicy {
    Forbidden,
    Allowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputConsumption {
    Streaming,
    SinglePassBatches,
    RewindableBatches,
    RandomAccess,
    FullyMaterialized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputProduction {
    Streaming,
    Batches,
    FullyMaterialized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortEditorSpec {
    Default,
    Hidden,
    InlineLiteral,
    SchemaColumns { allow_multiple: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSemantics {
    pub determinism: Determinism,
    pub purity: Purity,
    pub evaluation: EvaluationPolicy,
    pub cache: CachePolicy,
    pub effects: EffectSemantics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectSemantics {
    None,
    Ordered,
    Exclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Determinism {
    Deterministic,
    EnvironmentDependent,
    NonDeterministic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Purity {
    Pure,
    Effectful,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvaluationPolicy {
    DemandDriven,
    EagerWhenRegionEntered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CachePolicy {
    None,
    PerRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeScope {
    Any,
    Event,
    Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManagedNodeRole {
    EventBegin,
    FunctionEntry,
    FunctionReturn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidIdentity(String),
    DuplicatePortKey(PortKey),
    DuplicateTypeParameter(TypeParameterId),
    InvalidPortContract { key: PortKey, reason: &'static str },
    InvalidExecutionSemantics(&'static str),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIdentity(error) => f.write_str(error),
            Self::DuplicatePortKey(key) => write!(f, "duplicate port key '{key}'"),
            Self::DuplicateTypeParameter(id) => write!(f, "duplicate type parameter '{id}'"),
            Self::InvalidPortContract { key, reason } => {
                write!(f, "invalid port contract '{key}': {reason}")
            }
            Self::InvalidExecutionSemantics(reason) => {
                write!(f, "invalid execution semantics: {reason}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

fn parse_id<T>(value: Result<T, super::InvalidSemanticId>) -> Result<T, ProtocolError> {
    value.map_err(|error| ProtocolError::InvalidIdentity(error.to_string()))
}

fn parse_i18n_key(value: &str) -> Result<I18nKey, ProtocolError> {
    parse_id(I18nKey::new(value))
}

fn invalid_port(key: &PortKey, reason: &'static str) -> ProtocolError {
    ProtocolError::InvalidPortContract {
        key: key.clone(),
        reason,
    }
}

fn validate_port_contract(port: &PortSpec) -> Result<(), ProtocolError> {
    if let PortInstances::UserCreated {
        min,
        max: Some(max),
    } = port.instances
    {
        if min > max {
            return Err(invalid_port(
                &port.key,
                "user-created port minimum exceeds maximum",
            ));
        }
    }
    if let ConnectionsPerPort::Multiple { max: Some(0), .. } = port.connections {
        return Err(invalid_port(
            &port.key,
            "multiple connection maximum must be positive",
        ));
    }

    let is_data_input = port.kind == PortKind::Data && port.direction == PortDirection::Input;
    let is_data_output = port.kind == PortKind::Data && port.direction == PortDirection::Output;
    if port.input_binding.is_some() && !is_data_input {
        return Err(invalid_port(
            &port.key,
            "only data inputs may declare literal/default bindings",
        ));
    }
    if port.consumption.is_some() && !is_data_input {
        return Err(invalid_port(
            &port.key,
            "only data inputs may declare consumption",
        ));
    }
    if port.production.is_some() && !is_data_output {
        return Err(invalid_port(
            &port.key,
            "only data outputs may declare production",
        ));
    }
    if port.schema.is_some() && !is_data_output {
        return Err(invalid_port(
            &port.key,
            "only data outputs may declare a schema expression",
        ));
    }
    if port.kind != PortKind::Data && port.value_type != TypeExpr::Unknown {
        return Err(invalid_port(
            &port.key,
            "control and effect ports cannot declare a value type",
        ));
    }
    if let Some(binding) = &port.input_binding {
        if binding.literal_policy == LiteralPolicy::Forbidden && binding.default_value.is_some() {
            return Err(invalid_port(
                &port.key,
                "a forbidden literal policy cannot carry a default",
            ));
        }
        if let Some(default) = &binding.default_value {
            if default.value_type != port.value_type {
                return Err(invalid_port(
                    &port.key,
                    "typed default does not match the port value type",
                ));
            }
        }
    }
    Ok(())
}

fn validate_execution(execution: ExecutionSemantics) -> Result<(), ProtocolError> {
    match (execution.purity, execution.effects) {
        (Purity::Pure, EffectSemantics::None)
        | (Purity::Effectful, EffectSemantics::Ordered | EffectSemantics::Exclusive) => Ok(()),
        (Purity::Pure, _) => Err(ProtocolError::InvalidExecutionSemantics(
            "pure nodes cannot declare effects",
        )),
        (Purity::Effectful, EffectSemantics::None) => {
            Err(ProtocolError::InvalidExecutionSemantics(
                "effectful nodes must declare effect ordering",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::protocol::{TypeId, Value};

    fn key(value: &str) -> PortKey {
        PortKey::new(value).unwrap()
    }

    fn data_input() -> PortSpec {
        let value_type = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
        PortSpec {
            key: key("value"),
            label_key: I18nKey::new("nodes.test.ports.value").unwrap(),
            direction: PortDirection::Input,
            kind: PortKind::Data,
            value_type: value_type.clone(),
            instances: PortInstances::Declared,
            connections: ConnectionsPerPort::Single,
            input_binding: Some(InputBindingSpec {
                literal_policy: LiteralPolicy::Allowed,
                default_value: Some(TypedValue {
                    value_type,
                    value: Value::Integer(1),
                }),
            }),
            consumption: Some(InputConsumption::FullyMaterialized),
            production: None,
            editor: PortEditorSpec::InlineLiteral,
            schema: None,
        }
    }

    #[test]
    fn interface_rejects_duplicate_port_keys() {
        let port = data_input();
        assert!(matches!(
            NodeInterfaceProtocol::new(vec![port.clone(), port], vec![], vec![]),
            Err(ProtocolError::DuplicatePortKey(_))
        ));
    }

    #[test]
    fn rejects_bindings_and_consumption_on_output() {
        let mut port = data_input();
        port.direction = PortDirection::Output;
        assert!(matches!(
            NodeInterfaceProtocol::new(vec![port], vec![], vec![]),
            Err(ProtocolError::InvalidPortContract { .. })
        ));
    }

    #[test]
    fn rejects_mismatched_typed_default() {
        let mut port = data_input();
        port.input_binding
            .as_mut()
            .unwrap()
            .default_value
            .as_mut()
            .unwrap()
            .value_type = TypeExpr::Concrete(TypeId::new("core.string").unwrap());
        assert!(matches!(
            NodeInterfaceProtocol::new(vec![port], vec![], vec![]),
            Err(ProtocolError::InvalidPortContract { .. })
        ));
    }

    #[test]
    fn rejects_invalid_instance_and_connection_bounds() {
        let mut port = data_input();
        port.instances = PortInstances::UserCreated {
            min: 2,
            max: Some(1),
        };
        assert!(NodeInterfaceProtocol::new(vec![port], vec![], vec![]).is_err());

        let mut port = data_input();
        port.connections = ConnectionsPerPort::Multiple {
            max: Some(0),
            ordered: true,
        };
        assert!(NodeInterfaceProtocol::new(vec![port], vec![], vec![]).is_err());
    }

    #[test]
    fn rejects_purity_effect_mismatches() {
        let pure_effect = ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::PerRun,
            effects: EffectSemantics::Ordered,
        };
        assert!(validate_execution(pure_effect).is_err());
    }
}
