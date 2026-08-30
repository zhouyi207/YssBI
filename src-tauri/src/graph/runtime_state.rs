use std::sync::Arc;

#[cfg(test)]
use std::sync::{Barrier, Mutex};

use crate::graph::error::GraphMaterializationError;
use thiserror::Error;
use yss_graph_analysis::{GraphAnalysis, GraphAnalysisInput, analyze, projection_facts};
use yss_graph_analysis_contract::CompilationBasis;
use yss_graph_catalog::{BuiltinCatalog, CatalogResourceEntry, LocalizedCatalog};
use yss_graph_document::{GraphDocument, GraphResourcePath, NodeId, PortAddress};
use yss_graph_document_edit::{GraphDocumentPatch, validate_graph_document};
use yss_graph_editor::{
    CatalogMutationValidationSnapshot, ClipboardSubgraph, EditorGraphMutation, MutationConflict,
    export_subgraph, filter_compatible_catalog,
};
use yss_graph_registry::NodeRegistry;
use yss_graph_resource_contract::ResourceCatalogSnapshot;

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
    pub resource_catalog: Arc<ResourceCatalogSnapshot>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GraphRuntimeTestEvent {
    Bound,
    Materialized,
    CatalogComputed,
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct GraphRuntimeTestControl {
    state: Arc<Mutex<GraphRuntimeTestControlState>>,
}

#[cfg(test)]
#[derive(Default)]
struct GraphRuntimeTestControlState {
    events: Vec<GraphRuntimeTestEvent>,
    fail_next_materialization: bool,
    materialization_pause: Option<GraphRuntimeTestRendezvous>,
    catalog_pause: Option<GraphRuntimeTestRendezvous>,
}

#[cfg(test)]
#[derive(Clone)]
struct GraphRuntimeTestRendezvous {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
}

#[cfg(test)]
impl GraphRuntimeTestControl {
    pub(crate) fn fail_next_materialization(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fail_next_materialization = true;
    }

    pub(crate) fn pause_after_materialization(&self, entered: Arc<Barrier>, release: Arc<Barrier>) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .materialization_pause = Some(GraphRuntimeTestRendezvous { entered, release });
    }

    pub(crate) fn pause_after_catalog_compute(&self, entered: Arc<Barrier>, release: Arc<Barrier>) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .catalog_pause = Some(GraphRuntimeTestRendezvous { entered, release });
    }

    pub(crate) fn events(&self) -> Vec<GraphRuntimeTestEvent> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .events
            .clone()
    }

    fn record_bound(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .events
            .push(GraphRuntimeTestEvent::Bound);
    }

    fn before_materialization_return(&self) -> bool {
        let (failure, pause) = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.events.push(GraphRuntimeTestEvent::Materialized);
            (
                std::mem::replace(&mut state.fail_next_materialization, false),
                state.materialization_pause.clone(),
            )
        };
        wait_for_test_rendezvous(pause);
        failure
    }

    fn after_catalog_compute(&self) {
        let pause = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.events.push(GraphRuntimeTestEvent::CatalogComputed);
            state.catalog_pause.clone()
        };
        wait_for_test_rendezvous(pause);
    }
}

#[cfg(test)]
fn wait_for_test_rendezvous(rendezvous: Option<GraphRuntimeTestRendezvous>) {
    if let Some(rendezvous) = rendezvous {
        rendezvous.entered.wait();
        rendezvous.release.wait();
    }
}

pub struct GraphRuntimeState {
    epoch: GraphRuntimeEpoch,
    components: GraphRuntimeComponents,
    #[cfg(test)]
    test_control: Option<Arc<GraphRuntimeTestControl>>,
}

impl GraphRuntimeState {
    pub fn from_components(epoch: GraphRuntimeEpoch, components: GraphRuntimeComponents) -> Self {
        Self {
            epoch,
            components,
            #[cfg(test)]
            test_control: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        epoch: GraphRuntimeEpoch,
        components: GraphRuntimeComponents,
        control: GraphRuntimeTestControl,
    ) -> Self {
        Self {
            epoch,
            components,
            test_control: Some(Arc::new(control)),
        }
    }

    pub const fn epoch(&self) -> GraphRuntimeEpoch {
        self.epoch
    }

    pub fn accepts_basis(
        &self,
        basis: &CompilationBasis<yss_graph_document::GraphRevision>,
    ) -> bool {
        *basis.registry_fingerprint.as_bytes() == *self.components.registry.fingerprint().as_bytes()
    }

    pub fn resource_catalog(&self) -> &ResourceCatalogSnapshot {
        &self.components.resource_catalog
    }

    pub(crate) fn registry(&self) -> &NodeRegistry {
        self.components.registry.as_ref()
    }

    pub(crate) fn plan_editor_mutation(
        &self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        mutation: EditorGraphMutation,
        catalog: &CatalogMutationValidationSnapshot,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        mutation.into_patch_with_catalog_snapshot(
            graph_path,
            document,
            self.registry(),
            Some(catalog),
        )
    }

    pub(crate) fn export_subgraph(
        &self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        catalog: &CatalogMutationValidationSnapshot,
        node_ids: Vec<NodeId>,
    ) -> Result<ClipboardSubgraph, MutationConflict> {
        export_subgraph(graph_path, document, self.registry(), catalog, node_ids)
    }

    pub(crate) fn registry_fingerprint(&self) -> [u8; 32] {
        *self.components.registry.fingerprint().as_bytes()
    }

    pub(crate) fn analyze(
        &self,
        document: &GraphDocument,
        basis: &CompilationBasis<yss_graph_document::GraphRevision>,
    ) -> GraphAnalysis {
        let analysis = analyze(GraphAnalysisInput { document, basis });
        analysis.with_projection_facts(projection_facts(
            document,
            self.components.registry.as_ref(),
        ))
    }

    pub(crate) fn bind_open_graph(&self) {
        #[cfg(test)]
        if let Some(control) = &self.test_control {
            control.record_bound();
        }
    }

    pub(crate) fn materialize_open_candidate(
        &self,
        document: &GraphDocument,
    ) -> Result<Arc<GraphDocument>, GraphMaterializationError> {
        validate_graph_document(document).map_err(|_| GraphMaterializationError::invariant())?;
        let candidate = Arc::new(document.clone());
        #[cfg(test)]
        if let Some(control) = &self.test_control {
            if control.before_materialization_return() {
                return Err(GraphMaterializationError::invariant());
            }
        }
        Ok(candidate)
    }

    pub(crate) fn localized_catalog_with_resources(
        &self,
        resources: &[CatalogResourceEntry],
        locale: &str,
    ) -> LocalizedCatalog {
        let localized = self.components.catalog.localize_with_resources(
            self.components.registry.as_ref(),
            locale,
            resources,
        );
        #[cfg(test)]
        if let Some(control) = &self.test_control {
            control.after_catalog_compute();
        }
        localized
    }

    pub(crate) fn compatible_catalog_with_resources(
        &self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        source: &PortAddress,
        catalog: &ResourceCatalogSnapshot,
        resources: &[CatalogResourceEntry],
        locale: &str,
    ) -> Result<LocalizedCatalog, GraphRuntimeCatalogError> {
        let localized = self.components.catalog.localize_with_resources(
            self.components.registry.as_ref(),
            locale,
            resources,
        );
        let localized = filter_compatible_catalog(
            graph_path,
            document,
            self.components.registry.as_ref(),
            source,
            catalog,
            resources,
            localized,
        )
        .map_err(|_| GraphRuntimeCatalogError::SourceInvalid)?;
        #[cfg(test)]
        if let Some(control) = &self.test_control {
            control.after_catalog_compute();
        }
        Ok(localized)
    }
}

#[derive(Debug, Error)]
pub(crate) enum GraphRuntimeCatalogError {
    #[error("compatible source port is invalid")]
    SourceInvalid,
}
