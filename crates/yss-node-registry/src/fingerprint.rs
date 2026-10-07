use serde::{Deserialize, Serialize};
use std::fmt;
use yss_canonical_hash::{CanonicalEncodingError, hash_canonical};
use yss_node_protocol::NodeProtocol;

macro_rules! fingerprint {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name([u8; 32]);
        impl $name {
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }
            pub fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
            pub fn to_hex(&self) -> String {
                self.0.iter().map(|b| format!("{b:02x}")).collect()
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.to_hex())
                    .finish()
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.to_hex())
            }
        }
    };
}

fingerprint!(ProtocolFingerprint);
fingerprint!(RegistryFingerprint);

pub(crate) fn protocol_fingerprint(
    protocol: &NodeProtocol,
) -> Result<ProtocolFingerprint, CanonicalEncodingError> {
    hash_canonical(
        "yssbi.node-protocol.v1",
        &canonical_semantic_protocol(protocol),
    )
    .map(ProtocolFingerprint)
}

pub(crate) fn canonical_semantic_protocol(protocol: &NodeProtocol) -> serde_json::Value {
    serde_json::json!({
        "execution": &protocol.execution,
        "interface": {
            "ports": protocol.interface.ports.iter().map(|port| serde_json::json!({
                "key": port.key,
                "direction": port.direction,
                "valueType": port.value_type,
                "cardinality": port.cardinality,
                "connections": port.connections,
                "inputBinding": port.input_binding,
                "consumption": port.consumption,
                "production": port.production,
                "schema": port.schema,
            })).collect::<Vec<_>>(),
            "typeParameters": protocol.interface.type_parameters,
            "memberGroups": protocol.interface.member_groups,
        },
        "managedRole": &protocol.managed_role,
        "parameters": protocol.parameters.iter().map(|parameter| (&parameter.key, semantic_parameter(parameter))).collect::<std::collections::BTreeMap<_, _>>(),
        "scope": &protocol.scope,
        "typing": &protocol.typing,
        "typeId": &protocol.type_id,
    })
}

fn semantic_parameter(parameter: &yss_node_protocol::Parameter) -> serde_json::Value {
    use yss_node_protocol::ParameterEditorSpec;
    // Some editors also own validation/normalization. Preserve those contracts explicitly;
    // labels, placement and ordinary widget choices do not affect execution identity.
    let validation = match &parameter.editor {
        ParameterEditorSpec::Resource { kind } => serde_json::json!({ "resource": kind }),
        ParameterEditorSpec::SemanticDomain => serde_json::json!("semanticDomain"),
        ParameterEditorSpec::GraphConstant => serde_json::json!("graphConstant"),
        _ => serde_json::Value::Null,
    };
    serde_json::json!({
        "key": parameter.key,
        "valueType": parameter.value_type,
        "default": parameter.default_value,
        "constraints": parameter.constraints,
        "condition": parameter.visible_when,
        "validation": validation,
    })
}

pub(crate) fn registry_fingerprint<T: Serialize>(
    value: &T,
) -> Result<RegistryFingerprint, CanonicalEncodingError> {
    hash_canonical("yssbi.node-registry.v1", value).map(RegistryFingerprint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_node_protocol::*;

    #[test]
    fn semantic_identity_ignores_presentation_and_preserves_parameter_validation() {
        let field = Parameter {
            key: "count".parse().unwrap(),
            title_key: "test.count.title".parse().unwrap(),
            description_key: None,
            value_type: TypeExpr::Concrete("core.numeric".parse().unwrap()),
            default_value: None,
            constraints: vec![],
            editor: ParameterEditorSpec::Number,
            presentation: ParameterPresentation::DetailPanel,
            visible_when: None,
        };
        let mut protocol = NodeProtocol {
            type_id: "test.numeric.node".parse().unwrap(),
            catalog: NodeCatalogProtocol {
                title_key: "test.title".parse().unwrap(),
                documentation_key: None,
                aliases_key: None,
                category_id: "test".parse().unwrap(),
                icon_id: "test".parse().unwrap(),
                style_id: "test".parse().unwrap(),
                hidden: false,
            },
            interface: NodeInterfaceProtocol {
                ports: vec![PortSpec {
                    key: "result".parse().unwrap(),
                    title: "Result".into(),
                    direction: PortDirection::Output,
                    value_type: field.value_type.clone(),
                    cardinality: PortCardinality::Declared,
                    connections: ConnectionsPerPort::Single,
                    input_binding: None,
                    consumption: None,
                    production: Some(OutputProduction::FullyMaterialized),
                    editor: PortEditorSpec::Default,
                    schema: None,
                }]
                .into_boxed_slice(),
                type_parameters: Box::new([]),
                member_groups: Box::new([]),
            },
            parameters: Parameters::new([
                ParameterGroup::new("parameters", [field]),
                ParameterGroup::new(
                    "sampling",
                    [Parameter::number("sample_count").int().min(1).default(100)],
                ),
            ])
            .unwrap(),
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: ExecutionSemantics {
                determinism: Determinism::Deterministic,
                cache: CachePolicy::PerRun,
            },
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        };
        let original = protocol_fingerprint(&protocol).unwrap();
        protocol.interface.ports[0].title = "Translated result".into();
        protocol.parameters.groups[0].parameters[0].presentation =
            ParameterPresentation::InlineAndDetail;
        protocol.parameters.groups[0].parameters[0].title_key = "other.title".parse().unwrap();
        protocol.parameters.groups[0].key = "renamed".parse().unwrap();
        protocol.parameters.groups[0].title_key = "other.group".parse().unwrap();
        protocol.parameters.groups.swap(0, 1);
        assert_eq!(original, protocol_fingerprint(&protocol).unwrap());
        protocol.parameters.groups.swap(0, 1);
        protocol.parameters.groups[0].parameters[0]
            .constraints
            .push(ParameterConstraint::Positive);
        let constrained = protocol_fingerprint(&protocol).unwrap();
        assert_ne!(original, constrained);
        protocol.parameters.groups[0].parameters[0].visible_when = Some(ParameterCondition {
            key: "method".parse().unwrap(),
            values: vec![serde_json::from_value(serde_json::json!({"String": "WLS"})).unwrap()]
                .into_boxed_slice(),
        });
        assert_ne!(constrained, protocol_fingerprint(&protocol).unwrap());
    }
}
