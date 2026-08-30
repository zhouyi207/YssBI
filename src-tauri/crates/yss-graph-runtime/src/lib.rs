#![forbid(unsafe_code)]

use std::sync::Arc;

#[cfg(any(test, feature = "test-support"))]
use std::sync::{Barrier, Mutex};

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
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphRuntimeTestEvent {
    Materialized,
    CatalogComputed,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Default)]
pub struct GraphRuntimeTestControl {
    state: Arc<Mutex<GraphRuntimeTestControlState>>,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Default)]
struct GraphRuntimeTestControlState {
    events: Vec<GraphRuntimeTestEvent>,
    fail_next_materialization: bool,
    materialization_pause: Option<GraphRuntimeTestRendezvous>,
    catalog_pause: Option<GraphRuntimeTestRendezvous>,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
struct GraphRuntimeTestRendezvous {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
}

#[cfg(any(test, feature = "test-support"))]
impl GraphRuntimeTestControl {
    pub fn fail_next_materialization(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fail_next_materialization = true;
    }

    pub fn pause_after_materialization(&self, entered: Arc<Barrier>, release: Arc<Barrier>) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .materialization_pause = Some(GraphRuntimeTestRendezvous { entered, release });
    }

    pub fn pause_after_catalog_compute(&self, entered: Arc<Barrier>, release: Arc<Barrier>) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .catalog_pause = Some(GraphRuntimeTestRendezvous { entered, release });
    }

    pub fn events(&self) -> Vec<GraphRuntimeTestEvent> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .events
            .clone()
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

#[cfg(any(test, feature = "test-support"))]
fn wait_for_test_rendezvous(rendezvous: Option<GraphRuntimeTestRendezvous>) {
    if let Some(rendezvous) = rendezvous {
        rendezvous.entered.wait();
        rendezvous.release.wait();
    }
}

pub struct GraphRuntimeState {
    epoch: GraphRuntimeEpoch,
    components: GraphRuntimeComponents,
    #[cfg(any(test, feature = "test-support"))]
    test_control: Option<Arc<GraphRuntimeTestControl>>,
}

impl GraphRuntimeState {
    pub fn from_components(epoch: GraphRuntimeEpoch, components: GraphRuntimeComponents) -> Self {
        Self {
            epoch,
            components,
            #[cfg(any(test, feature = "test-support"))]
            test_control: None,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn new_for_test(
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

    fn registry(&self) -> &NodeRegistry {
        self.components.registry.as_ref()
    }

    pub fn plan_editor_mutation(
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

    pub fn export_subgraph(
        &self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        catalog: &CatalogMutationValidationSnapshot,
        node_ids: Vec<NodeId>,
    ) -> Result<ClipboardSubgraph, MutationConflict> {
        export_subgraph(graph_path, document, self.registry(), catalog, node_ids)
    }

    pub fn registry_fingerprint(&self) -> [u8; 32] {
        *self.components.registry.fingerprint().as_bytes()
    }

    pub fn analyze(
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

    pub fn materialize_open_candidate(
        &self,
        document: &GraphDocument,
    ) -> Result<Arc<GraphDocument>, GraphMaterializationError> {
        validate_graph_document(document).map_err(|_| GraphMaterializationError::invariant())?;
        let candidate = Arc::new(document.clone());
        #[cfg(any(test, feature = "test-support"))]
        if let Some(control) = &self.test_control
            && control.before_materialization_return()
        {
            return Err(GraphMaterializationError::invariant());
        }
        Ok(candidate)
    }

    pub fn localized_catalog_with_resources(
        &self,
        resources: &[CatalogResourceEntry],
        locale: &str,
    ) -> LocalizedCatalog {
        let localized = self.components.catalog.localize_with_resources(
            self.components.registry.as_ref(),
            locale,
            resources,
        );
        #[cfg(any(test, feature = "test-support"))]
        if let Some(control) = &self.test_control {
            control.after_catalog_compute();
        }
        localized
    }

    pub fn compatible_catalog_with_resources(
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
        .map_err(|_| GraphRuntimeCatalogError)?;
        #[cfg(any(test, feature = "test-support"))]
        if let Some(control) = &self.test_control {
            control.after_catalog_compute();
        }
        Ok(localized)
    }
}

#[derive(Debug, Error)]
#[error("compatible source port is invalid")]
pub struct GraphRuntimeCatalogError;

#[derive(Debug, Error)]
#[error("graph materialization invariant failed")]
pub struct GraphMaterializationError;

impl GraphMaterializationError {
    const fn invariant() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_graph_catalog::build_builtin_node_system;

    fn components() -> GraphRuntimeComponents {
        let builtin = build_builtin_node_system().expect("built-in graph system must be valid");
        GraphRuntimeComponents {
            registry: builtin.registry,
            catalog: builtin.catalog,
        }
    }

    #[test]
    fn epoch_preserves_the_session_generation() {
        let epoch = GraphRuntimeEpoch::from_existing(42);
        assert_eq!(epoch.get(), 42);
    }

    #[test]
    fn materialization_validates_and_owns_the_candidate() {
        let runtime =
            GraphRuntimeState::from_components(GraphRuntimeEpoch::from_existing(1), components());
        let document = GraphDocument::default();

        let candidate = runtime
            .materialize_open_candidate(&document)
            .expect("the empty document must satisfy graph invariants");

        assert_eq!(candidate.as_ref(), &document);
        assert!(!std::ptr::eq(candidate.as_ref(), &document));
    }

    #[test]
    fn materialization_fault_injection_is_explicit_and_observable() {
        let control = GraphRuntimeTestControl::default();
        control.fail_next_materialization();
        let runtime = GraphRuntimeState::new_for_test(
            GraphRuntimeEpoch::from_existing(1),
            components(),
            control.clone(),
        );

        assert!(
            runtime
                .materialize_open_candidate(&GraphDocument::default())
                .is_err()
        );
        assert_eq!(control.events(), [GraphRuntimeTestEvent::Materialized]);
    }
}
