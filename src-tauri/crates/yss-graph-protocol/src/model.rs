use super::{
    I18nKey, IconId, InterfaceResolverId, InvalidSemanticId, NodeCategoryId, NodeStyleId,
    NodeTypeId, ParameterKey, ParameterSchema, PortKey, SchemaExpr, TypeConstraint, TypeExpr,
    TypeParameterId, TypedValue,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeProtocol {
    pub type_id: NodeTypeId,
    pub catalog: NodeCatalogProtocol,
    pub interface: NodeInterfaceProtocol,
    pub parameters: ParameterSchema,
    #[serde(default)]
    pub instance_display: NodeInstanceDisplaySpec,
    pub execution: ExecutionSemantics,
    pub scope: NodeScope,
    pub managed_role: Option<ManagedNodeRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NodeInstanceDisplaySpec {
    #[default]
    Static,
    ResourceParameter {
        parameter: ParameterKey,
        kind: ResourceDisplayKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceDisplayKind {
    Function,
    Variable,
    Database,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCatalogProtocol {
    pub title_key: I18nKey,
    pub documentation_key: Option<I18nKey>,
    pub aliases_key: Option<I18nKey>,
    pub category_id: NodeCategoryId,
    pub icon_id: IconId,
    pub style_id: NodeStyleId,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeInterfaceProtocol {
    pub ports: Box<[PortSpec]>,
    pub type_parameters: Box<[TypeParameterId]>,
    pub type_constraints: Box<[TypeConstraint]>,
    #[serde(default)]
    pub member_groups: Box<[PortMemberGroupSpec]>,
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
            member_groups: Box::new([]),
        })
    }

    pub fn with_member_groups(
        mut self,
        member_groups: Vec<PortMemberGroupSpec>,
    ) -> Result<Self, ProtocolError> {
        validate_member_groups(&self.ports, &member_groups)?;
        self.member_groups = member_groups.into_boxed_slice();
        Ok(self)
    }

    pub fn member_group_for_template(&self, template: &PortKey) -> Option<&PortMemberGroupSpec> {
        self.member_groups
            .iter()
            .find(|group| group.templates.contains(template))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortMemberGroupSpec {
    pub templates: Box<[PortKey]>,
    pub min: u16,
    pub max: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortSpec {
    pub key: PortKey,
    /// Stable, non-localized display title supplied by the node definition.
    /// `key` remains the port identity; Markdown documentation mirrors this
    /// title manually and is not parsed as a runtime source.
    pub title: Box<str>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
#[serde(deny_unknown_fields)]
pub struct ExecutionSemantics {
    pub determinism: Determinism,
    pub cache: CachePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Determinism {
    Deterministic,
    EnvironmentDependent,
    NonDeterministic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CachePolicy {
    Disabled,
    PerRun,
    PerSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeScope {
    Any,
    Event,
    Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManagedNodeRole {
    FunctionEntry,
    FunctionReturn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidIdentity(String),
    InvalidSemanticId {
        value: Box<str>,
        source: InvalidSemanticId,
    },
    DuplicatePortKey(PortKey),
    DuplicateTypeParameter(TypeParameterId),
    InvalidPortContract {
        key: PortKey,
        reason: &'static str,
    },
    InvalidPortMemberGroup(&'static str),
    InvalidExecutionSemantics(&'static str),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIdentity(error) => f.write_str(error),
            Self::InvalidSemanticId { value, source } => {
                write!(f, "invalid protocol semantic ID '{value}': {source}")
            }
            Self::DuplicatePortKey(key) => write!(f, "duplicate port key '{key}'"),
            Self::DuplicateTypeParameter(id) => write!(f, "duplicate type parameter '{id}'"),
            Self::InvalidPortContract { key, reason } => {
                write!(f, "invalid port contract '{key}': {reason}")
            }
            Self::InvalidPortMemberGroup(reason) => {
                write!(f, "invalid port member group: {reason}")
            }
            Self::InvalidExecutionSemantics(reason) => {
                write!(f, "invalid execution semantics: {reason}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidSemanticId { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn invalid_port(key: &PortKey, reason: &'static str) -> ProtocolError {
    ProtocolError::InvalidPortContract {
        key: key.clone(),
        reason,
    }
}

fn validate_member_groups(
    ports: &[PortSpec],
    member_groups: &[PortMemberGroupSpec],
) -> Result<(), ProtocolError> {
    let mut grouped_templates = BTreeSet::new();
    for group in member_groups {
        if group.templates.len() < 2 {
            return Err(ProtocolError::InvalidPortMemberGroup(
                "a member group requires at least two templates",
            ));
        }
        if group.max.is_some_and(|max| group.min > max) {
            return Err(ProtocolError::InvalidPortMemberGroup(
                "member minimum exceeds maximum",
            ));
        }
        for template in &group.templates {
            if !grouped_templates.insert(template.clone()) {
                return Err(ProtocolError::InvalidPortMemberGroup(
                    "a template may belong to only one member group",
                ));
            }
            let Some(port) = ports.iter().find(|port| &port.key == template) else {
                return Err(ProtocolError::InvalidPortMemberGroup(
                    "member group references an unknown template",
                ));
            };
            if !matches!(
                port.instances,
                PortInstances::UserCreated { min: 0, max: None }
            ) {
                return Err(ProtocolError::InvalidPortMemberGroup(
                    "group templates must be unbounded user-created ports with zero per-template minimum",
                ));
            }
        }
    }
    Ok(())
}

fn validate_port_contract(port: &PortSpec) -> Result<(), ProtocolError> {
    if let PortInstances::UserCreated {
        min,
        max: Some(max),
    } = port.instances
        && min > max
    {
        return Err(invalid_port(
            &port.key,
            "user-created port minimum exceeds maximum",
        ));
    }
    if let ConnectionsPerPort::Multiple { max: Some(0), .. } = port.connections {
        return Err(invalid_port(
            &port.key,
            "multiple connection maximum must be positive",
        ));
    }

    let is_input = port.direction == PortDirection::Input;
    let is_output = port.direction == PortDirection::Output;
    if port.input_binding.is_some() && !is_input {
        return Err(invalid_port(
            &port.key,
            "only data inputs may declare literal/default bindings",
        ));
    }
    if port.consumption.is_some() && !is_input {
        return Err(invalid_port(
            &port.key,
            "only data inputs may declare consumption",
        ));
    }
    if port.production.is_some() && !is_output {
        return Err(invalid_port(
            &port.key,
            "only data outputs may declare production",
        ));
    }
    if port.schema.is_some() && !is_output {
        return Err(invalid_port(
            &port.key,
            "only data outputs may declare a schema expression",
        ));
    }
    if let Some(binding) = &port.input_binding {
        if binding.literal_policy == LiteralPolicy::Forbidden && binding.default_value.is_some() {
            return Err(invalid_port(
                &port.key,
                "a forbidden literal policy cannot carry a default",
            ));
        }
        if let Some(default) = &binding.default_value
            && default.value_type != port.value_type
        {
            return Err(invalid_port(
                &port.key,
                "typed default does not match the port value type",
            ));
        }
    }
    Ok(())
}

pub fn validate_execution(execution: ExecutionSemantics) -> Result<(), ProtocolError> {
    if execution.determinism == Determinism::NonDeterministic
        && execution.cache != CachePolicy::Disabled
    {
        return Err(ProtocolError::InvalidExecutionSemantics(
            "non-deterministic nodes cannot cache outputs",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TypeId, Value};

    fn key(value: &str) -> PortKey {
        PortKey::new(value).unwrap()
    }

    fn data_input() -> PortSpec {
        let value_type = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
        PortSpec {
            key: key("value"),
            title: "Value".into(),
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
    fn effective_cache_policy_serde_uses_canonical_names() {
        assert_eq!(
            serde_json::to_string(&CachePolicy::Disabled).unwrap(),
            "\"Disabled\""
        );
        assert_eq!(
            serde_json::to_string(&CachePolicy::PerSession).unwrap(),
            "\"PerSession\""
        );
        assert_eq!(
            serde_json::from_str::<CachePolicy>("\"PerSession\"").unwrap(),
            CachePolicy::PerSession
        );
    }

    #[test]
    fn rejects_caching_for_non_deterministic_nodes() {
        let non_deterministic = ExecutionSemantics {
            determinism: Determinism::NonDeterministic,
            cache: CachePolicy::PerRun,
        };
        assert!(validate_execution(non_deterministic).is_err());
    }
}
