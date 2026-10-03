//! Order semantic stages and publish one complete snapshot per resolution.
use crate::document_index::DocumentIndex;
use crate::node_projection::project_nodes;
use crate::parameter_projection::project_schema_parameter_editors;
use crate::port_projection::include_referenced_orphan_ports;
use crate::schema_resolution::resolve_graph_schemas;
use crate::{
    GraphDiagnosticFact, GraphDiagnosticLocation, GraphNodeSemanticFact, GraphResolutionOutcome,
    GraphResolutionStage, GraphResolvedInputBinding, GraphResolvedInputSource, GraphSchemaIssue,
    GraphSchemaState, GraphSemanticCache, GraphSemanticSnapshot, function_validation,
    graph_problem, semantic_validation, type_resolution,
};
use yss_graph_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{GraphDocument, PortRef};
use yss_graph_resource_contract::ResourceCatalogSnapshot;
use yss_node_protocol::{PortDirection, ResolvedType};
use yss_node_registry::NodeRegistry;

pub fn resolve_graph_semantics(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
) -> GraphSemanticSnapshot {
    resolve_graph_semantics_with_cache(
        document,
        registry,
        resources,
        &mut GraphSemanticCache::default(),
    )
}

pub fn resolve_graph_semantics_with_cache(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
    cache: &mut GraphSemanticCache,
) -> GraphSemanticSnapshot {
    resolve_graph_semantics_inner(document, registry, resources, cache, true)
}

pub(crate) fn resolve_graph_semantics_inner(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
    cache: &mut GraphSemanticCache,
    validate_functions: bool,
) -> GraphSemanticSnapshot {
    let index = DocumentIndex::new(document);
    let mut diagnostics = Vec::new();
    let resolved_schemas =
        resolve_graph_schemas(document, &index, registry, resources, &mut cache.schemas);
    let (mut nodes, internal_interface_node) = project_nodes(
        document,
        &index,
        registry,
        resources,
        &resolved_schemas,
        &mut diagnostics,
    );
    include_referenced_orphan_ports(document, &index, &mut nodes, &mut diagnostics);
    diagnostics.extend(type_resolution::resolve_node_types(
        document, &index, registry, &mut nodes, cache,
    ));
    for node in &mut nodes {
        project_schema_parameter_editors(node);
        validate_node_schema(document, node, &mut diagnostics);
        node.inputs = resolve_input_bindings(document, &index, node);
    }
    diagnostics.extend(semantic_validation::validate(
        document, &index, registry, &nodes,
    ));
    let function_resolution = if validate_functions {
        function_validation::resolve(document, registry, resources)
    } else {
        Default::default()
    };
    diagnostics.extend(function_resolution.diagnostics);
    if index.topological_order().is_none() {
        diagnostics.push(graph_problem(
            GraphDiagnosticKind::DependencyValueCycle,
            GraphDiagnosticLocation::Graph,
            std::iter::empty(),
        ));
    }
    let complete = !diagnostics.iter().any(|diagnostic| diagnostic.blocking);
    GraphSemanticSnapshot::new(
        nodes,
        diagnostics,
        if let Some(node_id) = internal_interface_node {
            GraphResolutionOutcome::InternalFailure {
                stage: GraphResolutionStage::Analysis,
                code: "graph.interface.unsupported_resolver".into(),
                node_id: Some(node_id),
            }
        } else if resolved_schemas.internal_failure().is_some() {
            GraphResolutionOutcome::InternalFailure {
                stage: GraphResolutionStage::Analysis,
                code: "graph.schema.unsupported_resolver".into(),
                node_id: resolved_schemas
                    .internal_failure()
                    .map(|address| address.node_id),
            }
        } else if let Some(failure) = function_resolution.internal_failure {
            failure
        } else if complete {
            GraphResolutionOutcome::Complete
        } else {
            GraphResolutionOutcome::Incomplete
        },
    )
    .with_functions(function_resolution.functions)
}

fn validate_node_schema(
    document: &GraphDocument,
    node: &mut GraphNodeSemanticFact,
    diagnostics: &mut Vec<GraphDiagnosticFact>,
) {
    for port in &mut node.ports {
        let requires_schema = port.schema.is_some()
            || matches!(port.type_state.exact(), Some(ResolvedType::Nominal(id)) if id.as_str() == "tabular.dataframe");
        if requires_schema && matches!(port.schema_state, GraphSchemaState::NotApplicable) {
            port.schema_state = GraphSchemaState::Pending(GraphSchemaIssue::UnresolvedUpstream);
        }
        let accepts_deferred = port.direction == PortDirection::Output
            || matches!(
                document.nodes[&node.node_id].node_type.as_str(),
                "yssbi.dataframe.dropna.rows"
                    | "yssbi.dataframe.dropna.columns"
                    | "yssbi.dataframe.limit"
                    | "yssbi.debug.view"
            );
        if requires_schema
            && !(accepts_deferred && matches!(port.schema_state, GraphSchemaState::Deferred))
            && port.schema_state.exact().is_none()
            && !matches!(port.schema_state, GraphSchemaState::InternalFailure(_))
        {
            diagnostics.push(graph_problem(
                GraphDiagnosticKind::InterfaceSchemaDependencyUnresolved,
                GraphDiagnosticLocation::Port(port.address.clone()),
                std::iter::empty(),
            ));
        }
    }
}

fn resolve_input_bindings(
    document: &GraphDocument,
    index: &DocumentIndex<'_>,
    node: &GraphNodeSemanticFact,
) -> Box<[GraphResolvedInputBinding]> {
    let mut inputs = Vec::new();
    for port in node
        .ports
        .iter()
        .filter(|port| port.direction == PortDirection::Input && !port.orphan)
    {
        let group = match &port.address.port {
            PortRef::Instance { instance_id, .. } => Some(*instance_id),
            PortRef::Declared { .. } => None,
        };
        let connections = index.input_connections(&port.address);
        if connections.is_empty() {
            if let Some(value) = document
                .input_states
                .get(&port.address)
                .and_then(|state| state.literal_override.as_ref())
                .or(port.protocol_default.as_ref())
            {
                inputs.push(GraphResolvedInputBinding {
                    address: port.address.clone(),
                    group,
                    source: GraphResolvedInputSource::Literal(value.clone()),
                });
            }
        } else {
            inputs.extend(
                connections
                    .iter()
                    .map(|connection| GraphResolvedInputBinding {
                        address: port.address.clone(),
                        group,
                        source: GraphResolvedInputSource::Output(connection.output.clone()),
                    }),
            );
        }
    }
    inputs.into_boxed_slice()
}
