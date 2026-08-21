use super::*;

pub struct RegistryNode<'a> {
    pub protocol: &'a NodeProtocol,
    pub protocol_fingerprint: ProtocolFingerprint,
    pub behavior: RegistryNodeBehavior<'a>,
}

#[derive(Clone, Copy)]
pub enum RegistryNodeBehavior<'a> {
    Leaf(&'a NodeImplementation),
    ProtocolOnly,
    Structural(StructuralNodeRole),
    Transparent(TransparentNodeRole),
}

impl RegistryNode<'_> {
    pub(super) fn implementation(&self) -> Option<&NodeImplementation> {
        match self.behavior {
            RegistryNodeBehavior::Leaf(implementation) => Some(implementation),
            RegistryNodeBehavior::ProtocolOnly
            | RegistryNodeBehavior::Structural(_)
            | RegistryNodeBehavior::Transparent(_) => None,
        }
    }

    pub(super) fn structural_role(&self) -> Option<StructuralNodeRole> {
        match self.behavior {
            RegistryNodeBehavior::Leaf(_)
            | RegistryNodeBehavior::ProtocolOnly
            | RegistryNodeBehavior::Transparent(_) => None,
            RegistryNodeBehavior::Structural(role) => Some(role),
        }
    }
}

/// The compiler registry resolves nodes and supplies the type facts required by analysis.
pub(super) struct CompilerNominalValidator<'a, R>(pub(super) &'a R);

impl<R: CompilerRegistry> crate::node_system::protocol::NominalParameterValidator
    for CompilerNominalValidator<'_, R>
{
    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        self.0.validate_nominal_parameter(type_id, value)
    }
}

pub trait CompilerRegistry: TypeEnvironment {
    fn fingerprint(&self) -> &RegistryFingerprint;
    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>>;

    fn validate_nominal_parameter(
        &self,
        _type_id: &TypeId,
        _value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        None
    }

    fn prepare_nominal_parameter(
        &self,
        _type_id: &TypeId,
        _value: &serde_json::Value,
    ) -> Option<Result<crate::node_system::registry::PreparedNominalValue, String>> {
        None
    }
}

impl TypeEnvironment for NodeRegistry {
    fn concrete_implements(&self, value_type: &TypeId, class: &TypeClassId) -> Option<bool> {
        self.types()
            .get(value_type)
            .map(|registration| registration.classes.contains(class))
    }

    fn constructor_arity(&self, constructor: &TypeConstructorId) -> Option<usize> {
        self.types()
            .constructor(constructor)
            .map(|registration| registration.arity as usize)
    }
}

impl CompilerRegistry for NodeRegistry {
    fn fingerprint(&self) -> &RegistryFingerprint {
        self.fingerprint()
    }

    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>> {
        let registered = self.get(node_type)?;
        let behavior = match (
            registered.implementation(),
            registered.structural_role(),
            registered.transparent_role(),
        ) {
            (Some(implementation), None, None) => RegistryNodeBehavior::Leaf(
                implementation
                    .as_any()
                    .downcast_ref::<NodeImplementation>()
                    .expect("registry freeze guarantees compiler lowering capability"),
            ),
            (None, None, None) => RegistryNodeBehavior::ProtocolOnly,
            (None, Some(role), None) => RegistryNodeBehavior::Structural(role),
            (None, None, Some(role)) => RegistryNodeBehavior::Transparent(role),
            _ => unreachable!("registry freeze guarantees one validated node behavior"),
        };
        Some(RegistryNode {
            protocol: registered.protocol(),
            protocol_fingerprint: self
                .catalog_manifest()
                .node_protocols
                .get(node_type)?
                .clone(),
            behavior,
        })
    }

    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        NodeRegistry::validate_nominal_parameter(self, type_id, value)
    }

    fn prepare_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<crate::node_system::registry::PreparedNominalValue, String>> {
        NodeRegistry::prepare_nominal_parameter(self, type_id, value)
    }
}
