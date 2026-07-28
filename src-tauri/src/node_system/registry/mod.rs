mod fingerprint;
mod model;
mod validation;

pub use fingerprint::{ProtocolFingerprint, RegistryFingerprint};
pub use model::{
    CatalogManifest, CategoryRegistration, CategoryRegistry, I18nManifest, ImplementationKind,
    LeafImplementation, NodeImplementation, NodeImplementationCapability, NodeRegistry,
    ProviderRegistration, RegisteredNode, StructuralNodeRole, TypeConstructorRegistration,
    TypeRegistration, TypeRegistry,
};
pub use validation::RegistryValidationError;

use crate::node_system::protocol::{NodeProtocol, NodeTypeId, ProtocolError, StaticNodeProtocol};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

impl NodeRegistry {
    pub fn protocol(&self, id: &NodeTypeId) -> Option<&NodeProtocol> {
        self.get(id).map(|node| node.protocol.as_ref())
    }
}

#[derive(Debug, Default)]
pub struct NodeRegistryBuilder {
    providers: Vec<ProviderRegistration>,
    provider_ids: BTreeSet<crate::node_system::protocol::ProviderId>,
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

    pub fn freeze(self) -> Result<NodeRegistry, NodeRegistrationError> {
        let parts = validation::validate(&self.providers)?;
        let protocol_fingerprints: BTreeMap<_, _> = parts
            .nodes
            .iter()
            .map(|(id, node)| {
                (
                    id.clone(),
                    fingerprint::protocol_fingerprint(&node.protocol),
                )
            })
            .collect();
        let canonical = canonical_registry(&self.providers, &protocol_fingerprints);
        let fingerprint = fingerprint::registry_fingerprint(&canonical);
        Ok(NodeRegistry {
            by_id: parts.nodes,
            type_index: parts.types,
            category_index: parts.categories,
            catalog_manifest: CatalogManifest {
                node_protocols: protocol_fingerprints,
                i18n: parts.i18n,
            },
            fingerprint,
        })
    }
}

impl RegisteredNode {
    pub fn leaf_static(
        protocol: &'static StaticNodeProtocol,
        implementation: impl Into<LeafImplementation>,
    ) -> Result<Self, ProtocolError> {
        Ok(Self::leaf(
            Arc::new(NodeProtocol::from_static(protocol)?),
            implementation,
        ))
    }

    pub fn structural_static(
        protocol: &'static StaticNodeProtocol,
        role: StructuralNodeRole,
    ) -> Result<Self, ProtocolError> {
        Ok(Self::structural(
            Arc::new(NodeProtocol::from_static(protocol)?),
            role,
        ))
    }
}

#[derive(Debug)]
pub enum NodeRegistrationError {
    InvalidProtocol(ProtocolError),
    InvalidRegistry(RegistryValidationError),
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
impl std::fmt::Display for NodeRegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProtocol(e) => e.fmt(f),
            Self::InvalidRegistry(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for NodeRegistrationError {}

pub fn canonical_semantic_protocol_snapshot(registry: &NodeRegistry) -> String {
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
    .expect("canonical semantic protocol snapshot is serializable")
}

pub fn i18n_inventory(registry: &NodeRegistry) -> String {
    serde_json::to_string_pretty(&registry.catalog_manifest.i18n.keys)
        .expect("i18n inventory is serializable")
}

fn canonical_registry(
    providers: &[ProviderRegistration],
    protocols: &BTreeMap<crate::node_system::protocol::NodeTypeId, ProtocolFingerprint>,
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
                    let behavior = match (&x.implementation, x.structural_role) {
                        (Some(implementation), None) => json!({
                            "implementationIdentity": implementation.implementation_identity(),
                            "kind": implementation.capability(),
                        }),
                        (None, Some(role)) => json!({
                            "kind": "Structural",
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
    json!({ "providers": entries })
}

#[cfg(test)]
mod tests;
