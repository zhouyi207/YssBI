#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

#[cfg(any(test, feature = "test-support"))]
use std::sync::Barrier;

use thiserror::Error;
use yss_graph_analysis::{
    GraphAnalysis, GraphSemanticCache, GraphSemanticSnapshot, analyze,
    resolve_graph_semantics_with_cache,
};
use yss_graph_analysis_contract::{
    CompilationBasis, CompileId, DiagnosticArguments, LocalizationLookup,
};
use yss_graph_catalog::{BuiltinCatalog, CatalogResourceEntry, LocalizedCatalog};
use yss_graph_compiler::{GraphCompilationInput, GraphCompileError, GraphCompiledPackage, compile};
use yss_graph_document::{GraphDocument, GraphResourcePath, NodeId, PortAddress};
use yss_graph_document_edit::{GraphDocumentPatch, validate_graph_document};
use yss_graph_editor::{
    CatalogCompatibilityError, CatalogMutationValidationSnapshot, ClipboardSubgraph,
    EditorGraphMutation, MutationConflict, export_subgraph, filter_compatible_catalog,
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
    compiled_drafts: Mutex<BTreeMap<GraphResourcePath, CachedGraphDraft>>,
    semantic_caches: Mutex<BTreeMap<GraphResourcePath, GraphSemanticCache>>,
    #[cfg(any(test, feature = "test-support"))]
    test_control: Option<Arc<GraphRuntimeTestControl>>,
}

#[derive(Clone)]
struct CachedGraphDraft {
    source_hash: [u8; 32],
    resource_catalog_fingerprint: [u8; 32],
    document: Arc<GraphDocument>,
    analysis: GraphAnalysis,
    package: GraphCompiledPackage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledGraphDraft {
    source_hash: [u8; 32],
    resource_catalog_fingerprint: [u8; 32],
    document: Arc<GraphDocument>,
    analysis: GraphAnalysis,
    package: GraphCompiledPackage,
}

impl CompiledGraphDraft {
    pub const fn source_hash(&self) -> &[u8; 32] {
        &self.source_hash
    }

    pub fn document(&self) -> &GraphDocument {
        &self.document
    }

    pub const fn resource_catalog_fingerprint(&self) -> &[u8; 32] {
        &self.resource_catalog_fingerprint
    }

    pub fn package(&self) -> &GraphCompiledPackage {
        &self.package
    }

    pub fn analysis(&self) -> &GraphAnalysis {
        &self.analysis
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphDraftCompilation {
    source_hash: [u8; 32],
    cache_hit: bool,
    analysis: GraphAnalysis,
}

impl GraphDraftCompilation {
    pub const fn source_hash(&self) -> &[u8; 32] {
        &self.source_hash
    }

    pub const fn cache_hit(&self) -> bool {
        self.cache_hit
    }

    pub fn analysis(&self) -> &GraphAnalysis {
        &self.analysis
    }
}

impl GraphRuntimeState {
    pub fn from_components(epoch: GraphRuntimeEpoch, components: GraphRuntimeComponents) -> Self {
        Self {
            epoch,
            components,
            compiled_drafts: Mutex::new(BTreeMap::new()),
            semantic_caches: Mutex::new(BTreeMap::new()),
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
            compiled_drafts: Mutex::new(BTreeMap::new()),
            semantic_caches: Mutex::new(BTreeMap::new()),
            test_control: Some(Arc::new(control)),
        }
    }

    pub const fn epoch(&self) -> GraphRuntimeEpoch {
        self.epoch
    }

    fn registry(&self) -> &NodeRegistry {
        self.components.registry.as_ref()
    }

    fn compile_graph(
        &self,
        document: &GraphDocument,
        semantics: &GraphSemanticSnapshot,
        graph: GraphResourcePath,
        compile_id: CompileId,
    ) -> Result<GraphCompiledPackage, GraphCompileError> {
        compile(GraphCompilationInput::new(
            document, semantics, graph, compile_id,
        ))
    }

    pub fn compile_draft(
        &self,
        document: &GraphDocument,
        graph: GraphResourcePath,
        resource_catalog: &ResourceCatalogSnapshot,
        basis: &CompilationBasis,
    ) -> Result<GraphDraftCompilation, GraphDraftCompilationError> {
        let source_hash = graph_draft_source_hash(
            document,
            &self.registry_fingerprint(),
            resource_catalog.fingerprint().as_bytes(),
        )?;
        if let Some(cached) = self
            .compiled_drafts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&graph)
            .filter(|cached| cached.source_hash == source_hash)
            .cloned()
        {
            return Ok(GraphDraftCompilation {
                source_hash,
                cache_hit: true,
                analysis: cached.analysis,
            });
        }
        let analysis = self.analyze_neutral(&graph, document, basis, resource_catalog);
        let semantics = analysis.semantic_snapshot();
        let compile_id = CompileId::new(u64::from_be_bytes(
            source_hash[..8]
                .try_into()
                .expect("SHA-256 prefix has exactly eight bytes"),
        ));
        let package = self
            .compile_graph(document, semantics, graph.clone(), compile_id)
            .map_err(GraphDraftCompilationError::Compile)?;
        self.compiled_drafts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                graph,
                CachedGraphDraft {
                    source_hash,
                    resource_catalog_fingerprint: *resource_catalog.fingerprint().as_bytes(),
                    document: Arc::new(document.clone()),
                    analysis: analysis.clone(),
                    package: package.clone(),
                },
            );
        Ok(GraphDraftCompilation {
            source_hash,
            cache_hit: false,
            analysis,
        })
    }

    pub fn compiled_draft(
        &self,
        graph: &GraphResourcePath,
        source_hash: &[u8; 32],
    ) -> Option<CompiledGraphDraft> {
        self.compiled_drafts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(graph)
            .filter(|cached| &cached.source_hash == source_hash)
            .map(|cached| CompiledGraphDraft {
                source_hash: cached.source_hash,
                resource_catalog_fingerprint: cached.resource_catalog_fingerprint,
                document: Arc::clone(&cached.document),
                analysis: cached.analysis.clone(),
                package: cached.package.clone(),
            })
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
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        basis: &CompilationBasis,
        resource_catalog: &ResourceCatalogSnapshot,
        resources: &[CatalogResourceEntry],
        locale: &str,
    ) -> GraphAnalysis {
        let analysis = self.analyze_neutral(graph_path, document, basis, resource_catalog);
        self.localize_analysis(document, analysis, resources, locale)
    }

    fn analyze_neutral(
        &self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        basis: &CompilationBasis,
        resource_catalog: &ResourceCatalogSnapshot,
    ) -> GraphAnalysis {
        let mut cache = self
            .semantic_caches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(graph_path)
            .unwrap_or_default();
        let snapshot = resolve_graph_semantics_with_cache(
            document,
            self.components.registry.as_ref(),
            resource_catalog,
            &mut cache,
        );
        self.semantic_caches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(graph_path.clone(), cache);
        analyze(basis, snapshot)
    }

    pub fn localize_analysis(
        &self,
        document: &GraphDocument,
        analysis: GraphAnalysis,
        resources: &[CatalogResourceEntry],
        locale: &str,
    ) -> GraphAnalysis {
        let snapshot = localize_semantic_snapshot(
            document,
            self.components.registry.as_ref(),
            resources,
            &self.components.catalog.localization(locale),
            analysis.semantic_snapshot().clone(),
        );
        analysis.with_semantic_snapshot(snapshot)
    }

    pub fn materialize_draft(
        &self,
        document: &GraphDocument,
        resources: &ResourceCatalogSnapshot,
    ) -> GraphDocument {
        yss_graph_analysis::materialize_derived_port_bindings(
            document,
            self.components.registry.as_ref(),
            resources,
        )
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
        .map_err(GraphRuntimeCatalogError::from)?;
        #[cfg(any(test, feature = "test-support"))]
        if let Some(control) = &self.test_control {
            control.after_catalog_compute();
        }
        Ok(localized)
    }
}

#[derive(Debug, Error)]
pub enum GraphDraftCompilationError {
    #[error("graph draft source hashing failed")]
    SourceHash(#[source] yss_canonical_hash::CanonicalEncodingError),
    #[error("graph draft compilation failed")]
    Compile(#[source] GraphCompileError),
}

fn graph_draft_source_hash(
    document: &GraphDocument,
    registry_fingerprint: &[u8; 32],
    resource_catalog_fingerprint: &[u8; 32],
) -> Result<[u8; 32], GraphDraftCompilationError> {
    let nodes = document
        .nodes
        .iter()
        .map(|(id, node)| (id, &node.node_type, &node.parameters))
        .collect::<Vec<_>>();
    let mut connections = document
        .connections
        .values()
        .map(|connection| (&connection.output, &connection.input, &connection.order))
        .collect::<Vec<_>>();
    connections.sort();
    let port_bindings = document.port_bindings.iter().collect::<Vec<_>>();
    let input_states = document.input_states.iter().collect::<Vec<_>>();
    yss_canonical_hash::hash_canonical(
        "yssbi.graph-draft-compilation.v1",
        &(
            nodes,
            port_bindings,
            connections,
            input_states,
            registry_fingerprint,
            resource_catalog_fingerprint,
        ),
    )
    .map_err(GraphDraftCompilationError::SourceHash)
}

fn localize_semantic_snapshot(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &[CatalogResourceEntry],
    localization: &dyn LocalizationLookup,
    snapshot: GraphSemanticSnapshot,
) -> GraphSemanticSnapshot {
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
    let nodes = snapshot.nodes().iter().cloned().map(|mut node_facts| {
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
    GraphSemanticSnapshot::new(
        nodes,
        snapshot.diagnostics().iter().cloned(),
        snapshot.outcome().clone(),
    )
}

#[derive(Debug, Error)]
pub enum GraphRuntimeCatalogError {
    #[error("compatible source port is invalid")]
    SourceInvalid,
}

impl From<CatalogCompatibilityError> for GraphRuntimeCatalogError {
    fn from(error: CatalogCompatibilityError) -> Self {
        match error {
            CatalogCompatibilityError::SourceInvalid => Self::SourceInvalid,
        }
    }
}

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
    use yss_graph_document::{DocumentNode, NodePosition, ParameterValues};
    use yss_graph_registry::RegistryFingerprint;

    fn components() -> GraphRuntimeComponents {
        let builtin = build_builtin_node_system().expect("built-in graph system must be valid");
        GraphRuntimeComponents {
            registry: builtin.registry,
            catalog: builtin.catalog,
        }
    }

    fn empty_resource_catalog() -> ResourceCatalogSnapshot {
        ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            yss_graph_resource_contract::ResourceCatalogFingerprint::from_bytes([0; 32]),
        )
    }

    fn basis(runtime: &GraphRuntimeState) -> CompilationBasis {
        CompilationBasis {
            registry_fingerprint: RegistryFingerprint::from_bytes(runtime.registry_fingerprint()),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
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
            registry_fingerprint: RegistryFingerprint::from_bytes(runtime.registry_fingerprint()),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        };

        let graph = GraphResourcePath::new("events/Localized.yssbi-event").unwrap();
        let analysis = runtime.analyze(
            &graph,
            &document,
            &basis,
            &empty_resource_catalog(),
            &[],
            "zh-CN",
        );
        let node = analysis
            .semantic_snapshot()
            .nodes()
            .first()
            .expect("the semantic snapshot includes the node");

        assert_eq!(node.title.as_ref(), "布尔常量");
    }

    #[test]
    fn dataframe_schema_compilation_expands_decompose_output_pins() {
        use yss_data_contract::DataType;
        use yss_graph_resource_contract::{
            ColumnSchema, DataSchema, GraphResourceId, ResourceCatalogFingerprint,
        };

        let runtime =
            GraphRuntimeState::from_components(GraphRuntimeEpoch::from_existing(1), components());
        let source_id = NodeId::new();
        let decompose_id = NodeId::new();
        let source_output = PortAddress::declared(
            source_id,
            "dataframe".parse().expect("built-in port key is valid"),
        );
        let decompose_input = PortAddress::declared(
            decompose_id,
            "dataframe".parse().expect("built-in port key is valid"),
        );
        let mut document = GraphDocument::default();
        document.nodes.insert(
            source_id,
            DocumentNode {
                id: source_id,
                node_type: "yssbi.dataframe.source.get"
                    .parse()
                    .expect("built-in node type is valid"),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::from([(
                    "dataframe"
                        .parse()
                        .expect("built-in parameter key is valid"),
                    serde_json::json!("databases/sales"),
                )]),
                user_label: None,
            },
        );
        document.nodes.insert(
            decompose_id,
            DocumentNode {
                id: decompose_id,
                node_type: "yssbi.dataframe.decompose"
                    .parse()
                    .expect("built-in node type is valid"),
                position: NodePosition { x: 300.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        let connection_id = yss_graph_document::ConnectionId::new();
        document.connections.insert(
            connection_id,
            yss_graph_document::DocumentConnection {
                id: connection_id,
                output: source_output.clone(),
                input: decompose_input.clone(),
                order: None,
            },
        );
        let resources = ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::from([(
                GraphResourceId::new("databases/sales"),
                DataSchema {
                    columns: vec![
                        ColumnSchema {
                            name: "customer_id".into(),
                            data_type: DataType::Int64,
                        },
                        ColumnSchema {
                            name: "amount".into(),
                            data_type: DataType::Float64,
                        },
                    ],
                },
            )]),
            ResourceCatalogFingerprint::from_bytes([7; 32]),
        );
        let basis = CompilationBasis {
            registry_fingerprint: RegistryFingerprint::from_bytes(runtime.registry_fingerprint()),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        };

        let mut document = runtime.materialize_draft(&document, &resources);
        for binding in document.port_bindings.values_mut() {
            if let yss_graph_document::DynamicPortBinding::Resolved { last_known, .. } = binding {
                last_known.label = "stale".into();
                last_known.value_type = Some(yss_graph_protocol::TypeExpr::Concrete(
                    yss_graph_protocol::TypeId::new("core.string").unwrap(),
                ));
            }
        }
        let graph = GraphResourcePath::new("events/Schema.yssbi-event").unwrap();
        let analysis = runtime.analyze(&graph, &document, &basis, &resources, &[], "en-US");
        let node = analysis
            .semantic_snapshot()
            .nodes()
            .iter()
            .find(|node| node.node_id == decompose_id)
            .expect("Decompose projection is available");
        let outputs = node
            .ports
            .iter()
            .filter(|port| port.direction == yss_graph_protocol::PortDirection::Output)
            .collect::<Vec<_>>();

        assert_eq!(outputs.len(), 2);
        assert_eq!(
            outputs
                .iter()
                .map(|port| port.label.as_ref())
                .collect::<Vec<_>>(),
            ["customer_id", "amount"]
        );
        assert_eq!(
            outputs
                .iter()
                .map(|port| port.type_state.exact())
                .collect::<Vec<_>>(),
            [
                Some(&yss_graph_protocol::ResolvedType::Applied {
                    constructor: yss_graph_protocol::TypeConstructorId::new(
                        yss_graph_protocol::DATA_SERIES_CONSTRUCTOR_ID,
                    )
                    .unwrap(),
                    arguments: Box::new([yss_graph_protocol::ResolvedType::Nominal(
                        yss_graph_protocol::TypeId::new("core.int64").unwrap(),
                    )]),
                }),
                Some(&yss_graph_protocol::ResolvedType::Applied {
                    constructor: yss_graph_protocol::TypeConstructorId::new(
                        yss_graph_protocol::DATA_SERIES_CONSTRUCTOR_ID,
                    )
                    .unwrap(),
                    arguments: Box::new([yss_graph_protocol::ResolvedType::Nominal(
                        yss_graph_protocol::TypeId::new("core.float64").unwrap(),
                    )]),
                }),
            ]
        );
        assert!(outputs.iter().all(|port| {
            port.address.is_instance()
                && matches!(
                    port.backing,
                    yss_graph_analysis::GraphPortBacking::DocumentInstance
                )
        }));
        assert_eq!(document.port_bindings.len(), 2);
        let rematerialized = runtime.materialize_draft(&document, &resources);
        assert_ne!(rematerialized, document);
        assert_eq!(
            runtime.materialize_draft(&rematerialized, &resources),
            rematerialized
        );
        assert_eq!(
            analysis
                .semantic_snapshot()
                .nodes()
                .iter()
                .find(|node| node.node_id == source_id)
                .and_then(|node| node.ports.first())
                .and_then(|port| port.resolved_schema.as_ref())
                .map(|schema| schema.fields.len()),
            Some(2)
        );
    }

    #[test]
    fn function_signature_compilation_materializes_call_ports() {
        use yss_data_contract::DataType;
        use yss_graph_resource_contract::{
            FunctionCatalogEntry, FunctionParameterContract, FunctionSignature,
            ResourceCatalogFingerprint,
        };

        let runtime =
            GraphRuntimeState::from_components(GraphRuntimeEpoch::from_existing(1), components());
        let function = GraphResourcePath::new("functions/Forecast.yssbi-function")
            .expect("test function path is valid");
        let node_id = NodeId::new();
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: "yssbi.project.function.call"
                    .parse()
                    .expect("built-in node type is valid"),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::from([(
                    "target".parse().expect("built-in parameter key is valid"),
                    serde_json::json!(function.as_str()),
                )]),
                user_label: None,
            },
        );
        let resources = ResourceCatalogSnapshot::new(
            BTreeMap::from([(
                function,
                FunctionCatalogEntry::new(FunctionSignature::new(
                    vec![
                        FunctionParameterContract::new(
                            yss_graph_document::FunctionParameterId::new("series"),
                            "Series",
                            DataType::DataSeries(Box::new(DataType::Float64)),
                        ),
                        FunctionParameterContract::new(
                            yss_graph_document::FunctionParameterId::new("horizon"),
                            "Horizon",
                            DataType::Int64,
                        ),
                    ],
                    Some(DataType::DataSeries(Box::new(DataType::Float64))),
                )),
            )]),
            BTreeMap::new(),
            BTreeMap::new(),
            ResourceCatalogFingerprint::from_bytes([8; 32]),
        );

        let materialized = runtime.materialize_draft(&document, &resources);
        let mut labels = materialized
            .port_bindings
            .values()
            .filter_map(|binding| match binding {
                yss_graph_document::DynamicPortBinding::Resolved { last_known, .. } => {
                    Some(last_known.label.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        labels.sort_unstable();

        assert_eq!(labels, ["Horizon", "Result", "Series"]);
        assert_eq!(
            runtime.materialize_draft(&materialized, &resources),
            materialized
        );
    }

    #[test]
    fn draft_compilation_cache_tracks_semantics_not_layout() {
        let runtime =
            GraphRuntimeState::from_components(GraphRuntimeEpoch::from_existing(1), components());
        let graph =
            GraphResourcePath::new("events/Cache.yssbi-event").expect("test graph path is valid");
        let node_id = NodeId::new();
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: "yssbi.constant.int64"
                    .parse()
                    .expect("built-in node type is valid"),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::from([(
                    "value".parse().expect("built-in parameter key is valid"),
                    serde_json::json!(7),
                )]),
                user_label: None,
            },
        );
        let resources = empty_resource_catalog();
        let compile_basis = basis(&runtime);

        let first = runtime
            .compile_draft(&document, graph.clone(), &resources, &compile_basis)
            .expect("initial draft compiles");
        assert!(!first.cache_hit());

        document.nodes.get_mut(&node_id).unwrap().position = NodePosition { x: 20.0, y: 40.0 };
        let layout_only = runtime
            .compile_draft(&document, graph.clone(), &resources, &compile_basis)
            .expect("layout-only draft compiles");
        assert!(layout_only.cache_hit());
        assert_eq!(layout_only.source_hash(), first.source_hash());

        document.nodes.get_mut(&node_id).unwrap().parameters.insert(
            "value".parse().expect("built-in parameter key is valid"),
            serde_json::json!(8),
        );
        let semantic_change = runtime
            .compile_draft(&document, graph, &resources, &compile_basis)
            .expect("updated draft compiles");
        assert!(!semantic_change.cache_hit());
        assert_ne!(semantic_change.source_hash(), first.source_hash());
    }
}
