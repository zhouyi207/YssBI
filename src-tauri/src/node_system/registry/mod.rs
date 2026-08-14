#![deny(unused_must_use)]

mod fingerprint;
mod model;
mod validation;

pub(crate) use fingerprint::hash_canonical;
pub use fingerprint::{CanonicalEncodingError, ProtocolFingerprint, RegistryFingerprint};
pub use model::{
    CatalogManifest, CategoryRegistration, CategoryRegistry, I18nManifest, ImplementationKind,
    LeafImplementation, NodeImplementation, NodeImplementationCapability, NodeRegistry,
    ProviderRegistration, RegisteredNode, StructuralNodeRole, TransparentNodeRole,
    TypeConstructorRegistration, TypeRegistration, TypeRegistry,
};
pub use validation::RegistryValidationError;

use crate::node_system::protocol::{NodeProtocol, NodeTypeId, ProtocolError, TypeId};
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_NOMINAL_REGISTRATION_ID: AtomicU64 = AtomicU64::new(1);

fn allocate_nominal_registration_id(allocator: &AtomicU64) -> Result<u64, NodeRegistrationError> {
    allocator
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            next.checked_add(1)
        })
        .map_err(|_| RegistryValidationError::NominalRegistrationIdExhausted.into())
}

#[derive(Clone)]
pub struct PreparedNominalValue {
    type_id: TypeId,
    codec_identity: TypeId,
    codec_version: u32,
    registration_id: u64,
    value: Arc<dyn Any + Send + Sync>,
}

impl PreparedNominalValue {
    pub fn type_id(&self) -> &TypeId {
        &self.type_id
    }

    pub fn codec_identity(&self) -> &TypeId {
        &self.codec_identity
    }

    pub const fn codec_version(&self) -> u32 {
        self.codec_version
    }
}

pub struct NominalValueHandle<T> {
    registration_id: u64,
    marker: PhantomData<fn() -> T>,
}

impl<T> std::fmt::Debug for NominalValueHandle<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NominalValueHandle")
            .field("registration_id", &self.registration_id)
            .finish_non_exhaustive()
    }
}

impl<T> Clone for NominalValueHandle<T> {
    fn clone(&self) -> Self {
        Self {
            registration_id: self.registration_id,
            marker: PhantomData,
        }
    }
}

impl<T: Any + Send + Sync> NominalValueHandle<T> {
    pub fn get<'a>(&self, value: &'a PreparedNominalValue) -> Option<&'a T> {
        (self.registration_id == value.registration_id)
            .then(|| value.value.downcast_ref())
            .flatten()
    }
}

impl std::fmt::Debug for PreparedNominalValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedNominalValue")
            .field("type_id", &self.type_id)
            .field("codec_identity", &self.codec_identity)
            .field("codec_version", &self.codec_version)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct NominalParameterValidator {
    identity: TypeId,
    version: u32,
    registration_id: u64,
    prepare:
        Arc<dyn Fn(&serde_json::Value) -> Result<Arc<dyn Any + Send + Sync>, String> + Send + Sync>,
}

impl NominalParameterValidator {
    fn new(
        identity: TypeId,
        version: u32,
        registration_id: u64,
        prepare: impl Fn(&serde_json::Value) -> Result<Arc<dyn Any + Send + Sync>, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            identity,
            version,
            registration_id,
            prepare: Arc::new(prepare),
        }
    }

    fn prepare(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Result<PreparedNominalValue, String> {
        Ok(PreparedNominalValue {
            type_id: type_id.clone(),
            codec_identity: self.identity.clone(),
            codec_version: self.version,
            registration_id: self.registration_id,
            value: (self.prepare)(value)?,
        })
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

impl crate::node_system::protocol::NominalParameterValidator for NodeRegistry {
    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        NodeRegistry::validate_nominal_parameter(self, type_id, value)
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
        self.prepare_nominal_parameter(id, value)
            .map(|result| result.map(|_| ()))
    }

    pub fn prepare_nominal_parameter(
        &self,
        id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<PreparedNominalValue, String>> {
        self.nominal_validators
            .get(id)
            .map(|validator| validator.prepare(id, value))
    }
}

#[derive(Debug, Default)]
pub struct NodeRegistryBuilder {
    providers: Vec<ProviderRegistration>,
    provider_ids: BTreeSet<crate::node_system::protocol::ProviderId>,
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
        let registration_id = allocate_nominal_registration_id(&NEXT_NOMINAL_REGISTRATION_ID)?;
        self.nominal_validators.insert(
            id,
            NominalParameterValidator::new(identity, version, registration_id, move |value| {
                validator(value)?;
                Ok(Arc::new(()))
            }),
        );
        Ok(())
    }

    pub fn register_nominal_codec<T>(
        &mut self,
        id: TypeId,
        identity: TypeId,
        version: u32,
        prepare: impl Fn(&serde_json::Value) -> Result<T, String> + Send + Sync + 'static,
    ) -> Result<NominalValueHandle<T>, NodeRegistrationError>
    where
        T: Any + Send + Sync + 'static,
    {
        if std::any::TypeId::of::<T>() == std::any::TypeId::of::<serde_json::Value>() {
            return Err(RegistryValidationError::RawJsonNominalPayload(id).into());
        }
        if self.nominal_validators.contains_key(&id) {
            return Err(RegistryValidationError::DuplicateNominalValidator(id).into());
        }
        let registration_id = allocate_nominal_registration_id(&NEXT_NOMINAL_REGISTRATION_ID)?;
        self.nominal_validators.insert(
            id,
            NominalParameterValidator::new(identity, version, registration_id, move |value| {
                prepare(value).map(|value| Arc::new(value) as Arc<dyn Any + Send + Sync>)
            }),
        );
        Ok(NominalValueHandle {
            registration_id,
            marker: PhantomData,
        })
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

pub fn canonical_semantic_protocol_snapshot(
    registry: &NodeRegistry,
) -> Result<String, CanonicalEncodingError> {
    let nodes = registry
        .iter()
        .map(|(id, node)| {
            serde_json::json!({
                "nodeTypeId": id,
                "protocol": fingerprint::canonical_semantic_protocol(&node.protocol),
                "protocolFingerprint": registry.catalog_manifest.node_protocols[id].to_hex(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&serde_json::json!({
        "format": "yssbi.semantic-node-protocol.v1",
        "nodes": nodes,
    }))
    .map_err(CanonicalEncodingError::from_serde)
}

pub fn i18n_inventory(registry: &NodeRegistry) -> Result<String, CanonicalEncodingError> {
    serde_json::to_string_pretty(&registry.catalog_manifest.i18n.keys)
        .map_err(CanonicalEncodingError::from_serde)
}

fn canonical_registry(
    providers: &[ProviderRegistration],
    protocols: &BTreeMap<crate::node_system::protocol::NodeTypeId, ProtocolFingerprint>,
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
                            "kind": implementation.capability(),
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

#[cfg(test)]
mod tests;
