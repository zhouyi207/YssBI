use super::{ProtocolFingerprint, RegistryFingerprint};
use crate::graph::protocol::{
    I18nKey, InterfaceResolverId, NodeCategoryId, NodeProtocol, NodeTypeId, ProviderId,
    SchemaResolverId, TypeClassId, TypeConstructorId, TypeId,
};
use serde::Serialize;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

/// Closed capability categories understood by registry validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ImplementationKind {
    CompilerLowering,
    Unsupported,
}

pub type NodeImplementationCapability = ImplementationKind;

/// An explicitly implemented behavior capability owned by another node-system layer.
///
/// There is deliberately no blanket implementation: arbitrary values must not become
/// executable node implementations merely because they are `Any + Send + Sync`.
pub trait NodeImplementation: Any + Send + Sync {
    fn capability(&self) -> ImplementationKind;
    fn implementation_identity(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}

/// A leaf behavior handle with no public arbitrary-value constructor.
#[derive(Clone)]
pub struct LeafImplementation(Arc<dyn NodeImplementation>);

impl LeafImplementation {
    pub(crate) fn from_arc(implementation: Arc<dyn NodeImplementation>) -> Self {
        Self(implementation)
    }

    pub(crate) fn capability(&self) -> ImplementationKind {
        self.0.capability()
    }

    pub(crate) fn implementation_identity(&self) -> &str {
        self.0.implementation_identity()
    }

    pub(crate) fn as_any(&self) -> &dyn Any {
        self.0.as_any()
    }

    #[cfg(test)]
    pub(super) fn new(implementation: impl NodeImplementation + 'static) -> Self {
        Self(Arc::new(implementation))
    }
}

impl fmt::Debug for LeafImplementation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LeafImplementation")
            .field("capability", &self.capability())
            .field("implementation_identity", &self.implementation_identity())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StructuralNodeRole {
    Sequence,
    Branch,
    Loop,
    Call,
    EventBegin,
    FunctionEntry,
    FunctionReturn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum TransparentNodeRole {
    Reroute,
}

#[derive(Clone)]
pub struct RegisteredNode {
    pub(super) protocol: Arc<NodeProtocol>,
    pub(super) implementation: Option<LeafImplementation>,
    pub(super) structural_role: Option<StructuralNodeRole>,
    pub(super) transparent_role: Option<TransparentNodeRole>,
}

impl RegisteredNode {
    pub fn leaf(
        protocol: Arc<NodeProtocol>,
        implementation: impl Into<LeafImplementation>,
    ) -> Self {
        Self {
            protocol,
            implementation: Some(implementation.into()),
            structural_role: None,
            transparent_role: None,
        }
    }

    pub fn structural(protocol: Arc<NodeProtocol>, role: StructuralNodeRole) -> Self {
        Self {
            protocol,
            implementation: None,
            structural_role: Some(role),
            transparent_role: None,
        }
    }

    pub fn transparent(protocol: Arc<NodeProtocol>, role: TransparentNodeRole) -> Self {
        Self {
            protocol,
            implementation: None,
            structural_role: None,
            transparent_role: Some(role),
        }
    }

    pub fn protocol(&self) -> &NodeProtocol {
        &self.protocol
    }

    pub fn implementation(&self) -> Option<&LeafImplementation> {
        self.implementation.as_ref()
    }

    pub fn structural_role(&self) -> Option<StructuralNodeRole> {
        self.structural_role
    }

    pub fn transparent_role(&self) -> Option<TransparentNodeRole> {
        self.transparent_role
    }
}

impl fmt::Debug for RegisteredNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredNode")
            .field("protocol", &self.protocol)
            .field("has_implementation", &self.implementation.is_some())
            .field("structural_role", &self.structural_role)
            .field("transparent_role", &self.transparent_role)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRegistration {
    pub id: TypeId,
    pub title_key: I18nKey,
    pub classes: BTreeSet<TypeClassId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeConstructorRegistration {
    pub id: TypeConstructorId,
    pub title_key: I18nKey,
    pub arity: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeRegistry {
    pub(super) types: BTreeMap<TypeId, TypeRegistration>,
    pub(super) constructors: BTreeMap<TypeConstructorId, TypeConstructorRegistration>,
    pub(super) classes: BTreeSet<TypeClassId>,
}

impl TypeRegistry {
    pub fn get(&self, id: &TypeId) -> Option<&TypeRegistration> {
        self.types.get(id)
    }
    pub fn constructor(&self, id: &TypeConstructorId) -> Option<&TypeConstructorRegistration> {
        self.constructors.get(id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&TypeId, &TypeRegistration)> {
        self.types.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryRegistration {
    pub id: NodeCategoryId,
    pub title_key: I18nKey,
    pub parent: Option<NodeCategoryId>,
    pub order: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CategoryRegistry {
    pub(super) categories: BTreeMap<NodeCategoryId, CategoryRegistration>,
}

impl CategoryRegistry {
    pub fn get(&self, id: &NodeCategoryId) -> Option<&CategoryRegistration> {
        self.categories.get(id)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&NodeCategoryId, &CategoryRegistration)> {
        self.categories.iter()
    }
}

/// Declares available stable keys, independently of localized text loading.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct I18nManifest {
    pub keys: BTreeSet<I18nKey>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogManifest {
    pub node_protocols: BTreeMap<NodeTypeId, ProtocolFingerprint>,
    pub i18n: I18nManifest,
}

#[derive(Debug, Clone)]
pub struct ProviderRegistration {
    pub provider: ProviderId,
    pub types: Box<[TypeRegistration]>,
    pub type_constructors: Box<[TypeConstructorRegistration]>,
    pub type_classes: Box<[TypeClassId]>,
    pub categories: Box<[CategoryRegistration]>,
    pub i18n: I18nManifest,
    pub interface_resolvers: Box<[InterfaceResolverId]>,
    pub schema_resolvers: Box<[SchemaResolverId]>,
    pub nodes: Box<[RegisteredNode]>,
}

impl ProviderRegistration {
    pub fn new(provider: ProviderId) -> Self {
        Self {
            provider,
            types: Box::new([]),
            type_constructors: Box::new([]),
            type_classes: Box::new([]),
            categories: Box::new([]),
            i18n: I18nManifest::default(),
            interface_resolvers: Box::new([]),
            schema_resolvers: Box::new([]),
            nodes: Box::new([]),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeRegistry {
    pub(super) by_id: BTreeMap<NodeTypeId, Arc<RegisteredNode>>,
    pub(super) node_providers: BTreeMap<NodeTypeId, ProviderId>,
    pub(super) type_index: TypeRegistry,
    pub(super) type_providers: BTreeMap<TypeId, ProviderId>,
    pub(super) category_index: CategoryRegistry,
    pub(super) catalog_manifest: CatalogManifest,
    pub(super) nominal_validators:
        BTreeMap<crate::graph::protocol::TypeId, super::NominalParameterValidator>,
    pub(super) fingerprint: RegistryFingerprint,
}

impl NodeRegistry {
    pub fn get(&self, id: &NodeTypeId) -> Option<&Arc<RegisteredNode>> {
        self.by_id.get(id)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&NodeTypeId, &Arc<RegisteredNode>)> {
        self.by_id.iter()
    }
    pub fn len(&self) -> usize {
        self.by_id.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
    pub fn types(&self) -> &TypeRegistry {
        &self.type_index
    }
    pub fn node_provider(&self, id: &NodeTypeId) -> Option<&ProviderId> {
        self.node_providers.get(id)
    }
    pub fn type_provider(&self, id: &TypeId) -> Option<&ProviderId> {
        self.type_providers.get(id)
    }
    pub fn categories(&self) -> &CategoryRegistry {
        &self.category_index
    }
    pub fn catalog_manifest(&self) -> &CatalogManifest {
        &self.catalog_manifest
    }
    pub fn fingerprint(&self) -> &RegistryFingerprint {
        &self.fingerprint
    }
}
