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
    CompilationBasis, CompileId, DiagnosticArguments, LocalizationLookup, ResourceKey,
    ResourceObservedState, ResourceVersion,
};
use yss_graph_catalog::{BuiltinCatalog, CatalogResourceEntry, LocalizedCatalog};
use yss_graph_compiler::{GraphCompilationInput, GraphCompileError, GraphCompiledPackage, compile};
use yss_graph_document::{
    DynamicPortBinding, GraphDocument, GraphResourcePath, LastKnownPortMetadata, NodeId, OrderKey,
    PortAddress,
};
use yss_graph_document_edit::{
    GraphDocumentOperation, GraphDocumentPatch, apply_graph_document_patch, validate_graph_document,
};
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
    artifact_id: [u8; 32],
    document: Arc<GraphDocument>,
    analysis: GraphAnalysis,
    package: GraphCompiledPackage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledGraphDraft {
    artifact_id: [u8; 32],
    document: Arc<GraphDocument>,
    analysis: GraphAnalysis,
    package: GraphCompiledPackage,
}

impl CompiledGraphDraft {
    pub const fn artifact_id(&self) -> &[u8; 32] {
        &self.artifact_id
    }

    pub fn document(&self) -> &GraphDocument {
        &self.document
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
    artifact_id: Option<[u8; 32]>,
    cache_hit: bool,
    analysis: GraphAnalysis,
}

impl GraphDraftCompilation {
    pub const fn artifact_id(&self) -> Option<&[u8; 32]> {
        self.artifact_id.as_ref()
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
        semantics: &GraphSemanticSnapshot,
        graph: GraphResourcePath,
        compile_id: CompileId,
    ) -> Result<GraphCompiledPackage, GraphCompileError> {
        compile(GraphCompilationInput::new(
            semantics
                .ready()
                .ok_or_else(|| GraphCompileError::InvalidGraph {
                    graph: graph.clone(),
                    code: yss_graph_compiler::GraphCompileErrorCode::LoweringInvariant,
                })?,
            graph,
            compile_id,
        ))
    }

    pub fn compile_draft(
        &self,
        document: &GraphDocument,
        graph: GraphResourcePath,
        resource_catalog: &ResourceCatalogSnapshot,
        basis: &CompilationBasis,
    ) -> Result<GraphDraftCompilation, GraphDraftCompilationError> {
        let analysis =
            self.resolve_graph_draft(&graph, document, basis, resource_catalog, &[], "en-US");
        if let yss_graph_analysis::GraphResolutionOutcome::InternalFailure { code, .. } =
            analysis.semantic_snapshot().outcome()
        {
            return Err(GraphDraftCompilationError::Resolution { code: code.clone() });
        }
        if analysis.semantic_snapshot().has_blocking_diagnostics()
            || !matches!(
                analysis.semantic_snapshot().outcome(),
                yss_graph_analysis::GraphResolutionOutcome::Complete
            )
        {
            return Ok(GraphDraftCompilation {
                artifact_id: None,
                cache_hit: false,
                analysis,
            });
        }
        let artifact_id = *analysis.semantic_input_hash();
        if self
            .compiled_drafts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&graph)
            .filter(|cached| cached.artifact_id == artifact_id)
            .is_some()
        {
            return Ok(GraphDraftCompilation {
                artifact_id: Some(artifact_id),
                cache_hit: true,
                analysis,
            });
        }
        let semantics = analysis.semantic_snapshot();
        let compile_id = CompileId::new(u64::from_be_bytes(
            artifact_id[..8]
                .try_into()
                .expect("SHA-256 prefix has exactly eight bytes"),
        ));
        let package = self
            .compile_graph(semantics, graph.clone(), compile_id)
            .map_err(GraphDraftCompilationError::Compile)?;
        self.compiled_drafts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                graph,
                CachedGraphDraft {
                    artifact_id,
                    document: Arc::new(document.clone()),
                    analysis: analysis.clone(),
                    package: package.clone(),
                },
            );
        Ok(GraphDraftCompilation {
            artifact_id: Some(artifact_id),
            cache_hit: false,
            analysis,
        })
    }

    pub fn compiled_draft(
        &self,
        graph: &GraphResourcePath,
        artifact_id: &[u8; 32],
    ) -> Option<CompiledGraphDraft> {
        self.compiled_drafts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(graph)
            .filter(|cached| &cached.artifact_id == artifact_id)
            .map(|cached| CompiledGraphDraft {
                artifact_id: cached.artifact_id,
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
        semantics: &GraphSemanticSnapshot,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        let mut candidate = document.clone();
        let mut operations = Vec::new();
        for address in mutation.referenced_ports() {
            let Some(port) = semantics.concrete_interface().port(address) else {
                continue;
            };
            if port.orphan {
                return Err(MutationConflict::Editor(
                    yss_graph_editor::EditorMutationError {
                        code: yss_graph_editor::EditorMutationErrorCode::GraphPortOrphan,
                        detail: "resource-derived port is orphaned".into(),
                    },
                ));
            }
            if let Some(previous) = candidate.port_bindings.get(address).cloned() {
                if let DynamicPortBinding::Orphan { origin, order, .. } = &previous {
                    let binding = DynamicPortBinding::Resolved {
                        origin: origin.clone(),
                        order: order.clone(),
                        last_known: LastKnownPortMetadata {
                            label: port.label.to_string(),
                            value_type: Some(port.accepted_type.clone()),
                        },
                    };
                    candidate
                        .port_bindings
                        .insert(address.clone(), binding.clone());
                    operations.push(GraphDocumentOperation::RemovePortBinding {
                        address: address.clone(),
                        binding: previous,
                    });
                    operations.push(GraphDocumentOperation::InsertPortBinding {
                        address: address.clone(),
                        binding,
                    });
                }
                continue;
            }
            let yss_graph_analysis::GraphPortBacking::ProjectedDerived { origin } = &port.backing
            else {
                continue;
            };
            let binding = DynamicPortBinding::Resolved {
                origin: origin.clone(),
                order: OrderKey::new(format!(
                    "{:010}",
                    semantics
                        .node(address.node_id)
                        .and_then(|node| node
                            .ports
                            .iter()
                            .position(|port| &port.address == address))
                        .unwrap_or(0)
                )),
                last_known: LastKnownPortMetadata {
                    label: port.label.to_string(),
                    value_type: Some(port.accepted_type.clone()),
                },
            };
            candidate
                .port_bindings
                .insert(address.clone(), binding.clone());
            operations.push(GraphDocumentOperation::InsertPortBinding {
                address: address.clone(),
                binding,
            });
        }
        let mutation_patch = mutation.into_patch_with_catalog_snapshot(
            graph_path,
            &candidate,
            self.registry(),
            Some(catalog),
        )?;
        apply_graph_document_patch(&mut candidate, &mutation_patch)?;
        operations.extend(mutation_patch.operations);
        for (address, binding) in &candidate.port_bindings {
            if matches!(binding, DynamicPortBinding::UserCreated { .. }) {
                continue;
            }
            let referenced = candidate.input_states.contains_key(address)
                || candidate.connections.values().any(|connection| {
                    &connection.output == address || &connection.input == address
                });
            if !referenced {
                operations.push(GraphDocumentOperation::RemovePortBinding {
                    address: address.clone(),
                    binding: binding.clone(),
                });
            }
        }
        Ok(GraphDocumentPatch::new(operations))
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

    pub fn resolve_graph_draft(
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
        let resources = resource_catalog.tracked();
        let snapshot = resolve_graph_semantics_with_cache(
            document,
            self.components.registry.as_ref(),
            &resources,
            &mut cache,
        );
        self.semantic_caches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(graph_path.clone(), cache);
        let dependencies = resources.dependencies();
        let mut resolved_basis = basis.clone();
        resolved_basis.resource_versions.clear();
        resolved_basis.resource_observations.clear();
        for (key, observed) in dependencies.entries() {
            let key = ResourceKey::new(key.storage_key());
            let observation = if let Some(fingerprint) = observed {
                let version = ResourceVersion::new(
                    fingerprint
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>(),
                );
                resolved_basis
                    .resource_versions
                    .insert(key.clone(), version.clone());
                ResourceObservedState::Present(version)
            } else {
                ResourceObservedState::Absent(None)
            };
            resolved_basis
                .resource_observations
                .insert(key, observation);
        }
        let hash = graph_semantic_input_hash(
            document,
            &self.registry_fingerprint(),
            &dependencies.fingerprint(),
        )
        .expect("validated graph semantic input is canonically serializable");
        analyze(&resolved_basis, snapshot.with_dependencies(dependencies))
            .with_semantic_input_hash(hash)
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
    #[error("graph semantic resolution failed: {code}")]
    Resolution { code: Box<str> },
    #[error("graph draft source hashing failed")]
    SourceHash(#[source] yss_canonical_hash::CanonicalEncodingError),
    #[error("graph draft compilation failed")]
    Compile(#[source] GraphCompileError),
}

fn graph_semantic_input_hash(
    document: &GraphDocument,
    registry_fingerprint: &[u8; 32],
    resource_catalog_fingerprint: &[u8; 32],
) -> Result<[u8; 32], GraphDraftCompilationError> {
    let document_hash = yss_graph_document::semantic_document_fingerprint(document)
        .map_err(GraphDraftCompilationError::SourceHash)?;
    yss_canonical_hash::hash_canonical(
        "yssbi.graph-artifact-input.v2",
        &(
            document_hash,
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
    snapshot.map_nodes(|mut node_facts| {
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
    })
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
        let analysis = runtime.resolve_graph_draft(
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
    fn dataframe_schema_resolves_decompose_outputs_without_changing_the_draft() {
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

        let graph = GraphResourcePath::new("events/Schema.yssbi-event").unwrap();
        let analysis =
            runtime.resolve_graph_draft(&graph, &document, &basis, &resources, &[], "en-US");
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
                    yss_graph_analysis::GraphPortBacking::ProjectedDerived { .. }
                )
        }));
        assert!(document.port_bindings.is_empty());
        let claimed_address = outputs[0].address.clone();
        let consumer = NodeId::new();
        document.nodes.insert(
            consumer,
            DocumentNode {
                id: consumer,
                node_type: "yssbi.debug.view".parse().unwrap(),
                position: NodePosition { x: 600.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        let before_claim =
            runtime.resolve_graph_draft(&graph, &document, &basis, &resources, &[], "en-US");
        let catalog = CatalogMutationValidationSnapshot {
            resources: BTreeMap::from([(
                yss_graph_catalog::CatalogResourcePath::new("databases/sales"),
                yss_graph_editor::CatalogMutationResource::Database {
                    authority_revision: 7,
                },
            )]),
        };
        let patch = runtime
            .plan_editor_mutation(
                &graph,
                &document,
                EditorGraphMutation::Connect {
                    output: claimed_address.clone(),
                    input: PortAddress::declared(consumer, "data".parse().unwrap()),
                    order: None,
                },
                &catalog,
                before_claim.semantic_snapshot(),
            )
            .unwrap();
        apply_graph_document_patch(&mut document, &patch).unwrap();
        assert_eq!(document.port_bindings.len(), 1);
        assert!(document.port_bindings.contains_key(&claimed_address));
        let changed_resources = ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::from([(
                GraphResourceId::new("databases/sales"),
                DataSchema {
                    columns: vec![ColumnSchema {
                        name: "amount".into(),
                        data_type: DataType::Float64,
                    }],
                },
            )]),
            ResourceCatalogFingerprint::from_bytes([9; 32]),
        );
        let orphaned = runtime.resolve_graph_draft(
            &graph,
            &document,
            &basis,
            &changed_resources,
            &[],
            "en-US",
        );
        assert!(
            orphaned
                .semantic_snapshot()
                .concrete_interface()
                .port(&claimed_address)
                .unwrap()
                .orphan
        );
        assert!(
            runtime
                .compile_draft(&document, graph.clone(), &changed_resources, &basis)
                .unwrap()
                .artifact_id()
                .is_none()
        );
        assert!(
            !runtime
                .resolve_graph_draft(&graph, &document, &basis, &resources, &[], "en-US")
                .semantic_snapshot()
                .concrete_interface()
                .port(&claimed_address)
                .unwrap()
                .orphan
        );
        assert_eq!(
            analysis
                .semantic_snapshot()
                .nodes()
                .iter()
                .find(|node| node.node_id == source_id)
                .and_then(|node| node.ports.first())
                .and_then(|port| port.schema_state.exact())
                .map(|schema| schema.fields.len()),
            Some(2)
        );
    }

    #[test]
    fn function_signature_resolves_stable_projected_call_ports() {
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

        let graph = GraphResourcePath::new("events/Call.yssbi-event").unwrap();
        let resolved = runtime.resolve_graph_draft(
            &graph,
            &document,
            &basis(&runtime),
            &resources,
            &[],
            "en-US",
        );
        let node = resolved.semantic_snapshot().node(node_id).unwrap();
        let mut labels = node
            .ports
            .iter()
            .map(|port| port.label.as_ref())
            .collect::<Vec<_>>();
        labels.sort_unstable();
        assert_eq!(labels, ["Horizon", "Result", "Series"]);
        assert!(document.port_bindings.is_empty());
        assert_eq!(
            runtime.resolve_graph_draft(
                &graph,
                &document,
                &basis(&runtime),
                &resources,
                &[],
                "en-US"
            ),
            resolved
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
        document.nodes.get_mut(&node_id).unwrap().user_label = Some("Renamed".into());
        let layout_only = runtime
            .compile_draft(&document, graph.clone(), &resources, &compile_basis)
            .expect("layout-only draft compiles");
        assert!(layout_only.cache_hit());
        assert_eq!(layout_only.artifact_id(), first.artifact_id());

        document.nodes.get_mut(&node_id).unwrap().parameters.insert(
            "value".parse().expect("built-in parameter key is valid"),
            serde_json::json!(8),
        );
        let semantic_change = runtime
            .compile_draft(&document, graph, &resources, &compile_basis)
            .expect("updated draft compiles");
        assert!(!semantic_change.cache_hit());
        assert_ne!(semantic_change.artifact_id(), first.artifact_id());

        // A signature label change keeps ABI/hash identity but must refresh editor facts.
        let function = GraphResourcePath::new("functions/Labels.yssbi-function").unwrap();
        let resources = |label: &str| {
            ResourceCatalogSnapshot::new(
                BTreeMap::from([(
                    function.clone(),
                    yss_graph_resource_contract::FunctionCatalogEntry::new(
                        yss_graph_resource_contract::FunctionSignature::new(
                            vec![yss_graph_resource_contract::FunctionParameterContract::new(
                                yss_graph_document::FunctionParameterId::new("parameter"),
                                label,
                                yss_data_contract::DataType::Int64,
                            )],
                            None,
                        ),
                    ),
                )]),
                BTreeMap::new(),
                BTreeMap::new(),
                yss_graph_resource_contract::ResourceCatalogFingerprint::from_bytes([0; 32]),
            )
        };
        let entry = document.nodes.get_mut(&node_id).unwrap();
        entry.node_type = "yssbi.project.function.entry".parse().unwrap();
        entry.parameters = ParameterValues::from([(
            "function".parse().unwrap(),
            serde_json::json!(function.as_str()),
        )]);
        let original = runtime
            .compile_draft(
                &document,
                function.clone(),
                &resources("Before"),
                &compile_basis,
            )
            .unwrap();
        let renamed = runtime
            .compile_draft(
                &document,
                function.clone(),
                &resources("After"),
                &compile_basis,
            )
            .unwrap();
        assert!(renamed.cache_hit());
        assert_eq!(renamed.artifact_id(), original.artifact_id());
        assert_eq!(
            renamed
                .analysis()
                .semantic_snapshot()
                .node(node_id)
                .unwrap()
                .ports[0]
                .label
                .as_ref(),
            "After"
        );
    }
}
