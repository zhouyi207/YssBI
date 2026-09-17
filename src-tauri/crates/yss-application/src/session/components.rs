use std::sync::Arc;

use yss_graph_runtime::GraphRuntimeComponents;
use yss_node_catalog::{BuiltinCatalog, BuiltinInitializationError};
use yss_node_kernel::{KernelBindingError, KernelRegistry, KernelRegistryBuilder};
use yss_node_protocol::{NodeTypingSpec, PortCardinality, PortDirection};
use yss_node_registry::NodeRegistry;

#[cfg(test)]
mod tests;

/// Immutable definitions and implementations reused when replacing a project session.
#[derive(Clone)]
pub struct NodeComponents {
    registry: Arc<NodeRegistry>,
    catalog: Arc<BuiltinCatalog>,
    kernels: Arc<KernelRegistry>,
}

#[derive(Debug, thiserror::Error)]
pub enum NodeCompositionError {
    #[error("built-in node definitions could not be constructed")]
    Builtins(#[from] BuiltinInitializationError),
    #[error("node {node} does not match execution kernel {kernel}")]
    Binding {
        node: yss_node_protocol::NodeTypeId,
        kernel: Box<str>,
        #[source]
        source: KernelBindingError,
    },
}

impl NodeComponents {
    pub fn new(
        registry: Arc<NodeRegistry>,
        catalog: Arc<BuiltinCatalog>,
        kernels: KernelRegistryBuilder,
    ) -> Result<Self, NodeCompositionError> {
        for (id, node) in registry.iter() {
            let Some(implementation) = node.implementation() else {
                continue;
            };
            let kernel = implementation.implementation_identity();
            // Keep definitions for existing graphs; the catalog marks missing kernels unavailable.
            let Some(contract) = kernels.contract(kernel) else {
                continue;
            };
            let protocol = node.protocol();
            let parameters = if matches!(protocol.typing, NodeTypingSpec::ConstantOutput { .. }) {
                vec!["value"]
            } else {
                protocol
                    .parameters
                    .parameters
                    .iter()
                    .map(|parameter| parameter.key.as_str())
                    .collect()
            };
            let (minimum, maximum) = protocol
                .interface
                .ports
                .iter()
                .filter(|port| port.direction == PortDirection::Output)
                .fold((0usize, 0usize), |(minimum, maximum), port| {
                    let (min, max) = match &port.cardinality {
                        PortCardinality::Declared => (1, 1),
                        PortCardinality::UserCreated { min, max } => {
                            (usize::from(*min), max.map_or(usize::MAX, usize::from))
                        }
                        PortCardinality::Derived { .. } => (0, usize::MAX),
                    };
                    (minimum.saturating_add(min), maximum.saturating_add(max))
                });
            contract
                .validate_binding(parameters, minimum..=maximum)
                .map_err(|source| NodeCompositionError::Binding {
                    node: id.clone(),
                    kernel: kernel.into(),
                    source,
                })?;
        }
        Ok(Self {
            registry,
            catalog,
            kernels: Arc::new(kernels.freeze()),
        })
    }

    pub fn builtins() -> Result<Self, NodeCompositionError> {
        let nodes = yss_node_catalog::build_builtin_node_system()?;
        Self::new(
            nodes.registry,
            nodes.catalog,
            KernelRegistryBuilder::with_builtins(),
        )
    }

    pub(super) fn graph(&self) -> GraphRuntimeComponents {
        GraphRuntimeComponents {
            registry: Arc::clone(&self.registry),
            catalog: Arc::clone(&self.catalog),
        }
    }

    pub(super) fn kernels(&self) -> Arc<KernelRegistry> {
        Arc::clone(&self.kernels)
    }

    pub(super) fn matches(&self, session: &super::ApplicationSession) -> bool {
        session.graph().registry_fingerprint() == *self.registry.fingerprint().as_bytes()
            && session.execution().kernels().fingerprint() == self.kernels.fingerprint()
    }
}
