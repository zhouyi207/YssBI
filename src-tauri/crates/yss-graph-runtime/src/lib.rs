#![forbid(unsafe_code)]

use std::{collections::BTreeMap, sync::Arc};

#[cfg(any(test, feature = "test-support"))]
use std::sync::{Barrier, Mutex};

use thiserror::Error;
use yss_graph_analysis::{
    GraphAnalysis, GraphAnalysisInput, GraphProjectionFacts, analyze, projection_facts,
};
use yss_graph_analysis_contract::{
    CompilationBasis, CompileId, DiagnosticArguments, LocalizationLookup,
};
use yss_graph_catalog::{BuiltinCatalog, CatalogResourceEntry, LocalizedCatalog};
use yss_graph_compiler::{GraphCompilationInput, GraphCompileError, GraphCompiledPackage, compile};
use yss_graph_document::{GraphDocument, GraphResourcePath, GraphRevision, NodeId, PortAddress};
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

    pub fn compile_graph(
        &self,
        document: &GraphDocument,
        expected_revision: GraphRevision,
        graph: GraphResourcePath,
        compile_id: CompileId,
    ) -> Result<GraphCompiledPackage, GraphCompileError> {
        compile(GraphCompilationInput::new(
            document,
            self.registry(),
            expected_revision,
            graph,
            compile_id,
        ))
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
        resources: &[CatalogResourceEntry],
        locale: &str,
    ) -> GraphAnalysis {
        let analysis = analyze(GraphAnalysisInput { document, basis });
        let facts = localize_projection_facts(
            document,
            self.components.registry.as_ref(),
            resources,
            &self.components.catalog.localization(locale),
            projection_facts(document, self.components.registry.as_ref()),
        );
        analysis.with_projection_facts(facts)
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

fn localize_projection_facts(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &[CatalogResourceEntry],
    localization: &dyn LocalizationLookup,
    facts: GraphProjectionFacts,
) -> GraphProjectionFacts {
    let arguments = DiagnosticArguments::new();
    let resource_names = resources
        .iter()
        .map(|entry| {
            (
                (entry.node_type_id.as_str(), entry.resource_path.as_str()),
                entry.name.as_ref(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let nodes = facts.nodes().iter().cloned().map(|mut node_facts| {
        let Some(protocol) = registry.protocol(&node_facts.node_type) else {
            return node_facts;
        };
        node_facts.title = localization.text(&protocol.catalog.title_key, &arguments);
        node_facts.instance_title = document
            .nodes
            .get(&node_facts.node_id)
            .and_then(|node| {
                node.parameters
                    .values()
                    .filter_map(|value| value.as_str())
                    .find_map(|resource_path| {
                        resource_names
                            .get(&(node.node_type.as_str(), resource_path))
                            .map(|name| Box::<str>::from(*name))
                    })
            })
            .or(node_facts.instance_title);

        for parameter in &mut node_facts.parameters {
            let Some(spec) = protocol
                .parameters
                .parameters
                .iter()
                .find(|spec| spec.key == parameter.key)
            else {
                continue;
            };
            parameter.title = localization.text(&spec.title_key, &arguments);
            parameter.description = spec
                .description_key
                .as_ref()
                .map(|key| localization.text(key, &arguments));
        }
        node_facts
    });
    GraphProjectionFacts::new(
        nodes,
        facts.diagnostics().iter().cloned(),
        facts.outcome().clone(),
    )
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
    use std::collections::BTreeMap;
    use yss_graph_analysis_contract::CompilationBasis;
    use yss_graph_catalog::build_builtin_node_system;
    use yss_graph_document::{DocumentNode, GraphRevision, NodePosition, ParameterValues};
    use yss_graph_registry::RegistryFingerprint;

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

    #[test]
    fn compilation_lowers_an_unbound_protocol_default_as_a_data_input() {
        let runtime =
            GraphRuntimeState::from_components(GraphRuntimeEpoch::from_existing(1), components());
        let node_id = NodeId::new();
        let node_type = runtime
            .registry()
            .iter()
            .map(|(node_type, _)| node_type)
            .find(|node_type| node_type.as_str() == "yssbi.debug.print")
            .cloned()
            .expect("the Print node is registered");
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type,
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        let graph = GraphResourcePath::new("events/Print.yssbi-event")
            .expect("fixture graph path is valid");

        let package = runtime
            .compile_graph(&document, document.revision, graph, CompileId::new(1))
            .expect("the protocol default makes Print executable");

        let operation = package
            .operations()
            .first()
            .expect("Print lowers to one operation");
        let input = operation
            .inputs()
            .first()
            .expect("Print has its default Message input");
        assert_eq!(input.kind(), yss_graph_compiler::GraphInputKind::Data);
        assert_eq!(input.port(), format!("{node_id}:message"));
        let yss_graph_compiler::GraphInputSource::Parameter(handle) = input.source() else {
            panic!("the unbound default must lower through the parameter bundle");
        };
        assert!(matches!(
            package.parameters().get(handle).map(|payload| payload.value()),
            Some(yss_graph_compiler::GraphParameterValue::Scalar(
                yss_graph_compiler::GraphParameterScalar::String(value)
            )) if value.as_ref() == "Hello, World!"
        ));
    }

    #[test]
    fn analysis_localizes_editor_node_titles() {
        let runtime =
            GraphRuntimeState::from_components(GraphRuntimeEpoch::from_existing(1), components());
        let node_id = NodeId::new();
        let node_type = runtime
            .registry()
            .iter()
            .map(|(node_type, _)| node_type)
            .find(|node_type| node_type.as_str() == "yssbi.constant.bool")
            .cloned()
            .expect("built-in node type is registered");
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type,
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        let basis = CompilationBasis {
            graph_revision: GraphRevision::INITIAL,
            registry_fingerprint: RegistryFingerprint::from_bytes(runtime.registry_fingerprint()),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        };

        let analysis = runtime.analyze(&document, &basis, &[], "zh-CN");
        let node = analysis
            .projection_facts()
            .and_then(|facts| facts.nodes().first())
            .expect("editor projection facts include the node");

        assert_eq!(node.title.as_ref(), "布尔常量");
    }
}
