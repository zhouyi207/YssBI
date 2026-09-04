#![deny(unused_must_use)]

mod fingerprint;
mod model;
mod validation;

pub use fingerprint::{ProtocolFingerprint, RegistryFingerprint};
pub use model::{
    CatalogManifest, CategoryRegistration, CategoryRegistry, I18nManifest, LeafImplementation,
    NodeRegistry, ProviderRegistration, RegisteredNode, StructuralNodeRole, TransparentNodeRole,
    TypeConstructorRegistration, TypeRegistration, TypeRegistry,
};
pub use validation::RegistryValidationError;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use yss_canonical_hash::CanonicalEncodingError;
use yss_graph_protocol::{NodeProtocol, NodeTypeId, ProtocolError, TypeClassId, TypeId};

type NominalValidatorFn = dyn Fn(&serde_json::Value) -> Result<(), String> + Send + Sync;

#[derive(Clone)]
struct NominalParameterValidator {
    identity: TypeId,
    version: u32,
    validate: Arc<NominalValidatorFn>,
}

impl NominalParameterValidator {
    fn new(
        identity: TypeId,
        version: u32,
        validate: impl Fn(&serde_json::Value) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            identity,
            version,
            validate: Arc::new(validate),
        }
    }

    fn validate(&self, value: &serde_json::Value) -> Result<(), String> {
        (self.validate)(value)
    }
}

impl std::fmt::Debug for NominalParameterValidator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NominalParameterValidator")
            .field("identity", &self.identity)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl yss_graph_protocol::TypeValidationContext for NodeRegistry {
    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        NodeRegistry::validate_nominal_parameter(self, type_id, value)
    }

    fn type_implements_class(&self, type_id: &TypeId, class: &TypeClassId) -> Option<bool> {
        self.types()
            .get(type_id)
            .map(|_| self.types().implements(type_id, class))
    }
}

impl NodeRegistry {
    pub fn protocol(&self, id: &NodeTypeId) -> Option<&NodeProtocol> {
        self.get(id).map(|node| node.protocol.as_ref())
    }

    pub fn has_nominal_parameter_validator(&self, id: &TypeId) -> bool {
        self.nominal_validators.contains_key(id)
    }

    pub fn validate_nominal_parameter(
        &self,
        id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        self.nominal_validators
            .get(id)
            .map(|validator| validator.validate(value))
    }
}

#[derive(Debug, Default)]
pub struct NodeRegistryBuilder {
    providers: Vec<ProviderRegistration>,
    provider_ids: BTreeSet<yss_graph_protocol::ProviderId>,
    nominal_validators: BTreeMap<TypeId, NominalParameterValidator>,
}

impl NodeRegistryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_provider(
        &mut self,
        provider: ProviderRegistration,
    ) -> Result<(), NodeRegistrationError> {
        if !self.provider_ids.insert(provider.provider.clone()) {
            return Err(RegistryValidationError::DuplicateProvider(provider.provider).into());
        }
        self.providers.push(provider);
        Ok(())
    }

    pub fn register_nominal_validator(
        &mut self,
        id: TypeId,
        identity: TypeId,
        version: u32,
        validator: impl Fn(&serde_json::Value) -> Result<(), String> + Send + Sync + 'static,
    ) -> Result<(), NodeRegistrationError> {
        if self.nominal_validators.contains_key(&id) {
            return Err(RegistryValidationError::DuplicateNominalValidator(id).into());
        }
        self.nominal_validators.insert(
            id,
            NominalParameterValidator::new(identity, version, validator),
        );
        Ok(())
    }

    pub fn freeze(self) -> Result<NodeRegistry, NodeRegistrationError> {
        let parts = validation::validate(&self.providers, &self.nominal_validators)?;
        let protocol_fingerprints: BTreeMap<_, _> = parts
            .nodes
            .iter()
            .map(|(id, node)| {
                Ok((
                    id.clone(),
                    fingerprint::protocol_fingerprint(&node.protocol)?,
                ))
            })
            .collect::<Result<_, CanonicalEncodingError>>()?;
        let canonical = canonical_registry(
            &self.providers,
            &protocol_fingerprints,
            &self.nominal_validators,
        );
        let fingerprint = fingerprint::registry_fingerprint(&canonical)?;
        Ok(NodeRegistry {
            by_id: parts.nodes,
            node_providers: parts.node_providers,
            type_index: parts.types,
            type_providers: parts.type_providers,
            category_index: parts.categories,
            catalog_manifest: CatalogManifest {
                node_protocols: protocol_fingerprints,
                i18n: parts.i18n,
            },
            nominal_validators: self.nominal_validators,
            fingerprint,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeRegistrationError {
    InvalidProtocol(ProtocolError),
    InvalidRegistry(RegistryValidationError),
    CanonicalEncoding(CanonicalEncodingError),
}
impl From<ProtocolError> for NodeRegistrationError {
    fn from(value: ProtocolError) -> Self {
        Self::InvalidProtocol(value)
    }
}
impl From<RegistryValidationError> for NodeRegistrationError {
    fn from(value: RegistryValidationError) -> Self {
        Self::InvalidRegistry(value)
    }
}
impl From<CanonicalEncodingError> for NodeRegistrationError {
    fn from(value: CanonicalEncodingError) -> Self {
        Self::CanonicalEncoding(value)
    }
}
impl std::fmt::Display for NodeRegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProtocol(e) => e.fmt(f),
            Self::InvalidRegistry(e) => e.fmt(f),
            Self::CanonicalEncoding(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for NodeRegistrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidProtocol(error) => Some(error),
            Self::InvalidRegistry(error) => Some(error),
            Self::CanonicalEncoding(error) => Some(error),
        }
    }
}

fn canonical_registry(
    providers: &[ProviderRegistration],
    protocols: &BTreeMap<yss_graph_protocol::NodeTypeId, ProtocolFingerprint>,
    nominal_validators: &BTreeMap<TypeId, NominalParameterValidator>,
) -> serde_json::Value {
    use serde_json::{Value, json};
    let mut sorted = providers.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.provider.cmp(&b.provider));
    let entries = sorted
        .into_iter()
        .map(|p| {
            let mut types = p
                .types
                .iter()
                .map(|x| json!([x.id, x.classes]))
                .collect::<Vec<_>>();
            types.sort_by_key(|x| x.to_string());
            let mut constructors = p
                .type_constructors
                .iter()
                .map(|x| json!([x.id, x.arity]))
                .collect::<Vec<_>>();
            constructors.sort_by_key(|x| x.to_string());
            let mut nodes = p
                .nodes
                .iter()
                .map(|x| {
                    let behavior = match (&x.implementation, x.structural_role, x.transparent_role)
                    {
                        (Some(implementation), None, None) => json!({
                            "implementationIdentity": implementation.implementation_identity(),
                            "kind": "CompilerLowering",
                        }),
                        (None, Some(role), None) => json!({
                            "kind": "Structural",
                            "role": format!("{role:?}"),
                        }),
                        (None, None, Some(role)) => json!({
                            "kind": "Transparent",
                            "role": format!("{role:?}"),
                        }),
                        _ => json!({ "kind": "Invalid" }),
                    };
                    json!([
                        x.protocol.type_id,
                        protocols[&x.protocol.type_id].to_hex(),
                        behavior,
                    ])
                })
                .collect::<Vec<_>>();
            nodes.sort_by_key(|x| x.to_string());
            json!({
                "provider": p.provider,
                "types": types,
                "constructors": constructors,
                "classes": p.type_classes.iter().collect::<BTreeSet<_>>(),
                "interface_resolvers": p.interface_resolvers.iter().collect::<BTreeSet<_>>(),
                "schema_resolvers": p.schema_resolvers.iter().collect::<BTreeSet<_>>(),
                "nodes": nodes,
            })
        })
        .collect::<Vec<Value>>();
    json!({
        "providers": entries,
        "nominalValidators": nominal_validators
            .iter()
            .map(|(type_id, validator)| json!([
                type_id,
                validator.identity,
                validator.version,
            ]))
            .collect::<Vec<_>>(),
    })
}
