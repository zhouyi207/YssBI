//! Immutable function programs captured from the same registry and resources as root analysis.
use std::{collections::BTreeMap, sync::Arc};
use yss_graph_analysis::{GraphAnalysis, GraphFunctionSemanticFact};
use yss_graph_document::GraphResourcePath;
use yss_graph_resource_contract::ResourceCatalogSnapshot;
use yss_node_registry::NodeRegistry;

#[derive(Debug)]
pub(crate) struct GraphFunctionLibrary {
    pub registry: NodeRegistry,
    pub resources: ResourceCatalogSnapshot,
    pub definitions: BTreeMap<GraphResourcePath, GraphFunctionSemanticFact>,
}

impl crate::plan::ExecutionPlanPackage {
    /// Attach captured code, not runtime values. Resource access during calls still uses
    /// the run's prepared grants, and Application revalidates the original capture on publication.
    pub fn with_functions(
        mut self,
        analysis: &GraphAnalysis,
        registry: &NodeRegistry,
        resources: &ResourceCatalogSnapshot,
    ) -> Result<Self, crate::graph_preparation::GraphPlanError> {
        if analysis.registry_fingerprint() != registry.fingerprint().as_bytes()
            || !resources.matches_dependencies(analysis.semantic_snapshot().dependencies())
        {
            return Err(crate::graph_preparation::GraphPlanError::InvalidGraph);
        }
        if !analysis.semantic_snapshot().functions().is_empty() {
            self.functions = Some(Arc::new(GraphFunctionLibrary {
                registry: registry.clone(),
                resources: resources.clone(),
                definitions: analysis.semantic_snapshot().functions().clone(),
            }));
        }
        Ok(self)
    }
}
