use std::sync::Arc;

use crate::execution::plan::PlanCompilationBasis;
use crate::graph::resource_catalog::ResourceCatalogSnapshot;
use crate::node_system::catalog::BuiltinCatalog;
use crate::node_system::compiler::ProjectCompileCoordinator;
use crate::node_system::registry::NodeRegistry;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GraphRuntimeEpoch(u64);

impl GraphRuntimeEpoch {
    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

pub struct GraphRuntimeComponents {
    pub registry: Arc<NodeRegistry>,
    pub catalog: Arc<BuiltinCatalog>,
    pub compiler: Arc<ProjectCompileCoordinator>,
    pub resource_catalog: Arc<ResourceCatalogSnapshot>,
}

pub struct GraphRuntimeState {
    epoch: GraphRuntimeEpoch,
    components: GraphRuntimeComponents,
}

impl GraphRuntimeState {
    pub fn from_components(epoch: GraphRuntimeEpoch, components: GraphRuntimeComponents) -> Self {
        Self { epoch, components }
    }

    pub const fn epoch(&self) -> GraphRuntimeEpoch {
        self.epoch
    }

    pub fn accepts_basis(&self, basis: &PlanCompilationBasis) -> bool {
        basis.registry_fingerprint().as_bytes()
            == *self.components.resource_catalog.fingerprint().as_bytes()
    }

    pub fn resource_catalog(&self) -> &ResourceCatalogSnapshot {
        &self.components.resource_catalog
    }
}
