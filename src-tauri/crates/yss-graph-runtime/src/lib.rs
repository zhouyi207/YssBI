#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

#[cfg(any(test, feature = "test-support"))]
use std::sync::Barrier;

use thiserror::Error;
use yss_graph_analysis::{
    GraphAnalysis, GraphSemanticSnapshot, analyze, resolve_graph_semantics_with_cache,
};
use yss_graph_analysis_contract::{
    GraphAnalysisBasis, ResourceKey, ResourceObservedState, ResourceVersion,
};
use yss_graph_diagnostics::{
    GRAPH_DIAGNOSTIC_DEFINITIONS, GraphDiagnosticDefinitionError,
    validate_graph_diagnostic_definitions,
};
use yss_graph_document::{
    DynamicPortBinding, GraphDocument, GraphResourcePath, LastKnownPortMetadata, NodeId, OrderKey,
    PortAddress,
};
use yss_graph_document::{GraphDocumentOperation, GraphDocumentPatch};
use yss_graph_document_edit::{apply_graph_document_patch, validate_graph_document};
use yss_graph_editor::{
    CatalogMutationValidationSnapshot, ClipboardSubgraph, EditorGraphMutation,
    EditorMutationContext, MutationConflict, SourcePort, export_subgraph,
    filter_compatible_catalog,
};
use yss_graph_resource_contract::ResourceCatalogSnapshot;
use yss_node_catalog::{BuiltinCatalog, CatalogResourceEntry, LocalizedCatalog};
use yss_node_protocol::PortDirection;
use yss_node_registry::{NodeRegistry, RegistryFingerprint};

mod semantic_cache;
use semantic_cache::{CachedGraphAnalysis, GraphResolutionCache, GraphResolutionCaches};

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

#[derive(Debug, Error)]
#[error("graph diagnostic definitions are invalid")]
pub struct GraphRuntimeInitializationError(#[from] GraphDiagnosticDefinitionError);

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
    semantic_caches: Mutex<GraphResolutionCaches>,
    #[cfg(any(test, feature = "test-support"))]
    test_control: Option<Arc<GraphRuntimeTestControl>>,
}

impl GraphRuntimeState {
    pub fn from_components(
        epoch: GraphRuntimeEpoch,
        components: GraphRuntimeComponents,
    ) -> Result<Self, GraphRuntimeInitializationError> {
        validate_graph_diagnostic_definitions(GRAPH_DIAGNOSTIC_DEFINITIONS)?;
        Ok(Self {
            epoch,
            components,
            semantic_caches: Mutex::new(GraphResolutionCaches::default()),
            #[cfg(any(test, feature = "test-support"))]
            test_control: None,
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn new_for_test(
        epoch: GraphRuntimeEpoch,
        components: GraphRuntimeComponents,
        control: GraphRuntimeTestControl,
    ) -> Self {
        let mut runtime = Self::from_components(epoch, components)
            .expect("graph diagnostic definitions must be valid");
        runtime.test_control = Some(Arc::new(control));
        runtime
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
        resolve: impl FnOnce() -> GraphAnalysis,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        let mut candidate = document.clone();
        let mut operations = Vec::new();
        let referenced_ports = mutation.referenced_ports();
        let analysis = (!referenced_ports.is_empty()).then(resolve);
        for address in referenced_ports {
            let semantics = analysis
                .as_ref()
                .expect("referenced ports require analysis")
                .semantic_snapshot();
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
        let mutation_patch = mutation.into_patch_with_context(
            graph_path,
            &candidate,
            self.registry(),
            EditorMutationContext {
                catalog: Some(catalog),
                semantics: analysis.as_ref().map(GraphAnalysis::semantic_snapshot),
            },
        )?;
        apply_graph_document_patch(&mut candidate, &mutation_patch)?;
        operations.extend(mutation_patch.operations);
        let referenced_ports = candidate
            .connections
            .values()
            .flat_map(|connection| [&connection.output, &connection.input])
            .chain(candidate.input_states.keys())
            .collect::<std::collections::BTreeSet<_>>();
        for (address, binding) in &candidate.port_bindings {
            if matches!(binding, DynamicPortBinding::UserCreated { .. }) {
                continue;
            }
            if !referenced_ports.contains(address) {
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
        document: &GraphDocument,
        catalog: &CatalogMutationValidationSnapshot,
        node_ids: Vec<NodeId>,
    ) -> Result<ClipboardSubgraph, MutationConflict> {
        export_subgraph(document, self.registry(), catalog, node_ids)
    }

    pub fn registry_fingerprint(&self) -> [u8; 32] {
        *self.components.registry.fingerprint().as_bytes()
    }

    pub fn resolve_graph_document(
        &self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        basis: &GraphAnalysisBasis,
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
        basis: &GraphAnalysisBasis,
        resource_catalog: &ResourceCatalogSnapshot,
    ) -> GraphAnalysis {
        let mut cache = self
            .semantic_caches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take(graph_path);
        let document_fingerprint = yss_graph_document::resolution_document_fingerprint(document)
            .expect("validated resolution inputs are serializable");
        if let Some(cached) = cache.analysis.as_ref().filter(|cached| {
            cached.document_fingerprint == document_fingerprint
                && cached.analysis.registry_fingerprint() == basis.registry_fingerprint.as_bytes()
                && cached.analysis.kernel_fingerprint() == &basis.kernel_fingerprint
                && cached.dependency_fingerprint
                    == resource_catalog.resolution_dependency_fingerprint(
                        cached.analysis.semantic_snapshot().dependencies(),
                    )
        }) {
            let analysis = cached.analysis.clone();
            self.retain_semantic_cache(graph_path, cache);
            return analysis;
        }
        let resources = resource_catalog.tracked();
        let snapshot = resolve_graph_semantics_with_cache(
            document,
            self.components.registry.as_ref(),
            &resources,
            &mut cache.nodes,
        );
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
            &basis.kernel_fingerprint,
            &dependencies.fingerprint(),
        )
        .expect("validated graph semantic input is canonically serializable");
        let dependency_fingerprint =
            resource_catalog.resolution_dependency_fingerprint(&dependencies);
        let analysis = analyze(&resolved_basis, snapshot.with_dependencies(dependencies))
            .with_semantic_input_hash(hash);
        cache.analysis = Some(CachedGraphAnalysis {
            document_fingerprint,
            dependency_fingerprint,
            analysis: analysis.clone(),
        });
        self.retain_semantic_cache(graph_path, cache);
        analysis
    }

    fn retain_semantic_cache(&self, graph_path: &GraphResourcePath, cache: GraphResolutionCache) {
        let retired = self
            .semantic_caches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .put(graph_path.clone(), cache);
        // Large snapshots and tabular constants are also released off-lock.
        drop(retired);
    }

    pub fn localize_analysis(
        &self,
        document: &GraphDocument,
        analysis: GraphAnalysis,
        resources: &[CatalogResourceEntry],
        locale: &str,
    ) -> GraphAnalysis {
        analysis.map_semantic_snapshot(|snapshot| {
            localize_semantic_snapshot(
                document,
                self.components.registry.as_ref(),
                resources,
                self.components.catalog.as_ref(),
                locale,
                snapshot,
            )
        })
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

    /// Annotate creation discovery without removing unavailable definitions.
    /// Structural and transparent nodes do not require a leaf kernel.
    pub fn annotate_catalog_availability(
        &self,
        catalog: &mut LocalizedCatalog,
        supports: impl Fn(&str) -> bool,
    ) {
        let available: std::collections::BTreeSet<&str> = self
            .registry()
            .iter()
            .filter(|(_, node)| {
                node.implementation()
                    .is_none_or(|implementation| supports(implementation.implementation_identity()))
            })
            .map(|(id, _)| id.as_str())
            .collect();
        for item in &mut catalog.items {
            item.available = available.contains(item.node_type_id.as_ref());
        }
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
        let basis = GraphAnalysisBasis {
            kernel_fingerprint: [0; 32],
            registry_fingerprint: RegistryFingerprint::from_bytes(self.registry_fingerprint()),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        };
        let analysis =
            self.resolve_graph_document(graph_path, document, &basis, catalog, resources, locale);
        let node = analysis
            .semantic_snapshot()
            .node(source.node_id)
            .ok_or(GraphRuntimeCatalogError::SourceInvalid)?;
        let port = node
            .ports
            .iter()
            .find(|port| &port.address == source)
            .filter(|port| !port.orphan)
            .ok_or(GraphRuntimeCatalogError::SourceInvalid)?;
        if port.direction == PortDirection::Output
            && matches!(
                port.schema_state,
                yss_graph_analysis::GraphSchemaState::Unavailable(_)
            )
        {
            return Err(GraphRuntimeCatalogError::SourceInvalid);
        }
        let source = SourcePort {
            address: source.clone(),
            direction: port.direction,
            value_type: port.connection_type(),
        };
        let localized = self.components.catalog.localize_with_resources(
            self.components.registry.as_ref(),
            locale,
            resources,
        );
        let localized = filter_compatible_catalog(
            graph_path,
            self.components.registry.as_ref(),
            &source,
            catalog,
            resources,
            localized,
        );
        #[cfg(any(test, feature = "test-support"))]
        if let Some(control) = &self.test_control {
            control.after_catalog_compute();
        }
        Ok(localized)
    }
}

fn graph_semantic_input_hash(
    document: &GraphDocument,
    registry_fingerprint: &[u8; 32],
    kernel_fingerprint: &[u8; 32],
    resource_catalog_fingerprint: &[u8; 32],
) -> Result<[u8; 32], yss_canonical_hash::CanonicalEncodingError> {
    let document_hash = yss_graph_document::semantic_document_fingerprint(document)?;
    yss_canonical_hash::hash_canonical(
        "yssbi.graph-semantic-input.v1",
        &(
            document_hash,
            registry_fingerprint,
            kernel_fingerprint,
            resource_catalog_fingerprint,
        ),
    )
}

fn localize_semantic_snapshot(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &[CatalogResourceEntry],
    catalog: &BuiltinCatalog,
    locale: &str,
    snapshot: GraphSemanticSnapshot,
) -> GraphSemanticSnapshot {
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
        node_facts.title = catalog.text(locale, &protocol.catalog.title_key);
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
                .iter()
                .find(|spec| spec.key == parameter.key)
            else {
                continue;
            };
            parameter.title = catalog.text(locale, &spec.title_key);
            parameter.description = spec
                .description_key
                .as_ref()
                .map(|key| catalog.text(locale, key));
            if let Some(
                yss_graph_analysis::GraphParameterConfigurationFact::ProjectColumns {
                    unavailable_reason,
                    ..
                }
                | yss_graph_analysis::GraphParameterConfigurationFact::FilterPredicate {
                    unavailable_reason,
                    ..
                },
            ) = &mut parameter.configuration
                && let Some(key) = unavailable_reason
                    .as_ref()
                    .and_then(|key| yss_node_protocol::I18nKey::new(key.clone()).ok())
            {
                *unavailable_reason = Some(catalog.text(locale, &key));
            }
        }
        for group in &mut node_facts.parameter_groups {
            if let Some(spec) = protocol
                .parameters
                .groups
                .iter()
                .find(|spec| spec.key == group.key)
            {
                group.title = catalog.text(locale, &spec.title_key);
                group.description = spec
                    .description_key
                    .as_ref()
                    .map(|key| catalog.text(locale, key));
            }
        }

        node_facts
    })
}

#[derive(Debug, Error)]
pub enum GraphRuntimeCatalogError {
    #[error("compatible source port is invalid")]
    SourceInvalid,
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
mod resolution_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use yss_graph_analysis_contract::GraphAnalysisBasis;
    use yss_graph_document::{DocumentNode, NodePosition, ParameterValues};
    use yss_node_catalog::build_builtin_node_system;
    use yss_node_registry::RegistryFingerprint;

    pub(super) fn components() -> GraphRuntimeComponents {
        let builtin = build_builtin_node_system().expect("built-in graph system must be valid");
        GraphRuntimeComponents {
            registry: builtin.registry,
            catalog: builtin.catalog,
        }
    }

    pub(super) fn basis(runtime: &GraphRuntimeState) -> GraphAnalysisBasis {
        GraphAnalysisBasis {
            kernel_fingerprint: [0; 32],
            registry_fingerprint: RegistryFingerprint::from_bytes(runtime.registry_fingerprint()),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        }
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
    fn function_signature_resolves_stable_projected_call_ports() {
        use yss_data_contract::ValueType;
        use yss_graph_resource_contract::{
            FunctionCatalogEntry, FunctionParameterContract, FunctionSignature,
            ResourceCatalogFingerprint,
        };

        let runtime =
            GraphRuntimeState::from_components(GraphRuntimeEpoch::from_existing(1), components())
                .unwrap();
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
                            ValueType::DataSeries(Box::new(ValueType::Scalar(
                                yss_data_contract::SemanticType::Numeric,
                            ))),
                        ),
                        FunctionParameterContract::new(
                            yss_graph_document::FunctionParameterId::new("horizon"),
                            "Horizon",
                            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                        ),
                    ],
                    Some(ValueType::DataSeries(Box::new(ValueType::Scalar(
                        yss_data_contract::SemanticType::Numeric,
                    )))),
                )),
            )]),
            BTreeMap::new(),
            ResourceCatalogFingerprint::from_bytes([8; 32]),
        );

        let graph = GraphResourcePath::new("events/Call.yssbi-event").unwrap();
        let resolved = runtime.resolve_graph_document(
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
        for port in &node.ports {
            let compatible = runtime
                .compatible_catalog_with_resources(
                    &graph,
                    &document,
                    &port.address,
                    &resources,
                    &[],
                    "en-US",
                )
                .expect("unclaimed function pins are valid catalog query sources");
            assert!(
                !compatible
                    .items
                    .iter()
                    .any(|item| item.node_type_id.as_ref() == "yssbi.logic.not")
            );
        }
        assert!(document.port_bindings.is_empty());
        assert_eq!(
            runtime.resolve_graph_document(
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
}
