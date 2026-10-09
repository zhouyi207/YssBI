use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use yss_canonical_hash::{CanonicalEncodingError, hash_canonical};
use yss_node_protocol::{
    NodeInterfaceProtocol, NodeProtocol, Parameter, ParameterEditorSpec, Parameters, PortSpec,
};

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
                hex::encode(self.0)
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
    hash_canonical("yssbi.node-protocol.v1", &SemanticProtocol(protocol)).map(ProtocolFingerprint)
}

struct SemanticProtocol<'a>(&'a NodeProtocol);

impl Serialize for SemanticProtocol<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let protocol = self.0;
        let mut fields = serializer.serialize_struct("SemanticProtocol", 7)?;
        fields.serialize_field("execution", &protocol.execution)?;
        fields.serialize_field("interface", &SemanticInterface(&protocol.interface))?;
        fields.serialize_field("managedRole", &protocol.managed_role)?;
        fields.serialize_field("parameters", &SemanticParameters(&protocol.parameters))?;
        fields.serialize_field("scope", &protocol.scope)?;
        fields.serialize_field("typing", &protocol.typing)?;
        fields.serialize_field("typeId", &protocol.type_id)?;
        fields.end()
    }
}

struct SemanticInterface<'a>(&'a NodeInterfaceProtocol);

impl Serialize for SemanticInterface<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let interface = self.0;
        let mut fields = serializer.serialize_struct("SemanticInterface", 3)?;
        fields.serialize_field("ports", &SemanticPorts(&interface.ports))?;
        fields.serialize_field("typeParameters", &interface.type_parameters)?;
        fields.serialize_field("memberGroups", &interface.member_groups)?;
        fields.end()
    }
}

struct SemanticPorts<'a>(&'a [PortSpec]);

impl Serialize for SemanticPorts<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.iter().map(SemanticPort))
    }
}

struct SemanticPort<'a>(&'a PortSpec);

impl Serialize for SemanticPort<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let port = self.0;
        let mut fields = serializer.serialize_struct("SemanticPort", 9)?;
        fields.serialize_field("key", &port.key)?;
        fields.serialize_field("direction", &port.direction)?;
        fields.serialize_field("valueType", &port.value_type)?;
        fields.serialize_field("cardinality", &port.cardinality)?;
        fields.serialize_field("connections", &port.connections)?;
        fields.serialize_field("inputBinding", &port.input_binding)?;
        fields.serialize_field("consumption", &port.consumption)?;
        fields.serialize_field("production", &port.production)?;
        fields.serialize_field("schema", &port.schema)?;
        fields.end()
    }
}

struct SemanticParameters<'a>(&'a Parameters);

impl Serialize for SemanticParameters<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Canonical Hash owns key ordering; parameter groups only own presentation order.
        let mut entries = serializer.serialize_map(None)?;
        for parameter in self.0.iter() {
            entries.serialize_entry(&parameter.key, &SemanticParameter(parameter))?;
        }
        entries.end()
    }
}

struct SemanticParameter<'a>(&'a Parameter);

impl Serialize for SemanticParameter<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let parameter = self.0;
        let mut fields = serializer.serialize_struct("SemanticParameter", 6)?;
        fields.serialize_field("key", &parameter.key)?;
        fields.serialize_field("valueType", &parameter.value_type)?;
        fields.serialize_field("default", &parameter.default_value)?;
        fields.serialize_field("constraints", &parameter.constraints)?;
        fields.serialize_field("condition", &parameter.visible_when)?;
        fields.serialize_field("validation", &SemanticValidation(&parameter.editor))?;
        fields.end()
    }
}

struct SemanticValidation<'a>(&'a ParameterEditorSpec);

impl Serialize for SemanticValidation<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // These editors also own validation/normalization; ordinary widgets only affect display.
        match self.0 {
            ParameterEditorSpec::Resource { kind } => {
                let mut fields = serializer.serialize_struct("ResourceValidation", 1)?;
                fields.serialize_field("resource", kind)?;
                fields.end()
            }
            ParameterEditorSpec::SemanticDomain => serializer.serialize_str("semanticDomain"),
            ParameterEditorSpec::GraphConstant => serializer.serialize_str("graphConstant"),
            _ => serializer.serialize_none(),
        }
    }
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
        assert_eq!(
            original.to_hex(),
            "4e0e70e78c22868362a499a02c054810fbf5b6700460630ec342a2a2812718f6"
        );
        for editor in [
            ParameterEditorSpec::Auto,
            ParameterEditorSpec::Hidden,
            ParameterEditorSpec::Text { multiline: true },
            ParameterEditorSpec::Toggle,
            ParameterEditorSpec::Select,
        ] {
            protocol.parameters.groups[0].parameters[0].editor = editor;
            assert_eq!(original, protocol_fingerprint(&protocol).unwrap());
        }
        for (editor, expected) in [
            (
                ParameterEditorSpec::SemanticDomain,
                "9bc25d2c142b3f25691eaf4d88826f1b0858f7aa19c35040004be3bf57d25f2b",
            ),
            (
                ParameterEditorSpec::GraphConstant,
                "0e0dc2a04513cbc14e99dda89cee9cf8a809c12fdf3a8aa05b02d19f437f5b69",
            ),
            (
                ParameterEditorSpec::Resource {
                    kind: ResourceDisplayKind::Function,
                },
                "360dc0560e076da17b35adb3e27fb15f967f584fc3613787707c64fcbed819cc",
            ),
            (
                ParameterEditorSpec::Resource {
                    kind: ResourceDisplayKind::Database,
                },
                "fc710a12c200fd5320ab33e7179804880406ba8478cf38c98af047533a98ea25",
            ),
        ] {
            protocol.parameters.groups[0].parameters[0].editor = editor;
            assert_eq!(protocol_fingerprint(&protocol).unwrap().to_hex(), expected);
        }
        protocol.parameters.groups[0].parameters[0].editor = ParameterEditorSpec::Number;
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
