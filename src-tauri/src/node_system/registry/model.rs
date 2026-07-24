use super::{ProtocolFingerprint, RegistryFingerprint};
use crate::node_system::protocol::{
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

#[derive(Clone)]
pub struct RegisteredNode {
    pub(crate) protocol: Arc<NodeProtocol>,
    pub(crate) implementation: Option<LeafImplementation>,
    pub(crate) structural_role: Option<StructuralNodeRole>,
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
        }
    }

    pub fn structural(protocol: Arc<NodeProtocol>, role: StructuralNodeRole) -> Self {
        Self {
            protocol,
            implementation: None,
            structural_role: Some(role),
        }
    }
}

impl fmt::Debug for RegisteredNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredNode")
            .field("protocol", &self.protocol)
            .field("has_implementation", &self.implementation.is_some())
            .field("structural_role", &self.structural_role)
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
    pub(crate) types: BTreeMap<TypeId, TypeRegistration>,
    pub(crate) constructors: BTreeMap<TypeConstructorId, TypeConstructorRegistration>,
    pub(crate) classes: BTreeSet<TypeClassId>,
}

impl TypeRegistry {
    pub fn get(&self, id: &TypeId) -> Option<&TypeRegistration> {
        self.types.get(id)
    }
    pub fn constructor(&self, id: &TypeConstructorId) -> Option<&TypeConstructorRegistration> {
        self.constructors.get(id)
    }
    pub fn contains_class(&self, id: &TypeClassId) -> bool {
        self.classes.contains(id)
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
    pub(crate) categories: BTreeMap<NodeCategoryId, CategoryRegistration>,
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
    pub(crate) by_id: BTreeMap<NodeTypeId, Arc<RegisteredNode>>,
    pub(crate) type_index: TypeRegistry,
    pub(crate) category_index: CategoryRegistry,
    pub(crate) catalog_manifest: CatalogManifest,
    pub(crate) fingerprint: RegistryFingerprint,
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
