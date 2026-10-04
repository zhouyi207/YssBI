//! Map evaluated result metadata into graph-scoped, versioned resolution inputs.
use crate::session::ApplicationSession;
use std::collections::BTreeMap;
use yss_graph_analysis::{GraphSchemaObservation, GraphSchemaObservations};
use yss_graph_document::{GraphDocument, GraphResourcePath, PortAddress};
use yss_node_kernel::RuntimeValue;
use yss_node_protocol::{
    PortCardinality, PortDirection, RelationalScalarType, SchemaColumnRef, SchemaField,
};

pub(super) fn capture(
    captured: &ApplicationSession,
    graph: &GraphResourcePath,
    document: &GraphDocument,
    resources: &yss_graph_resource_contract::ResourceCatalogSnapshot,
) -> GraphSchemaObservations {
    // Reuse Analysis's member identity for functions; never parse opaque plan addresses.
    let addresses = document
        .nodes
        .values()
        .flat_map(|node| {
            captured
                .graph()
                .registry()
                .protocol(&node.node_type)
                .into_iter()
                .flat_map(move |protocol| {
                    protocol
                        .interface
                        .ports
                        .iter()
                        .filter(|port| {
                            port.direction == PortDirection::Output
                                && matches!(port.cardinality, PortCardinality::Declared)
                        })
                        .map(move |port| PortAddress::declared(node.id, port.key.clone()))
                })
        })
        .chain(yss_graph_analysis::function_output_addresses(
            document,
            captured.graph().registry(),
            resources,
        ))
        .map(|address| (address.to_string(), address))
        .collect::<BTreeMap<_, _>>();
    captured
        .execution()
        .result_schema_candidates(graph.as_str())
        .into_iter()
        .filter_map(|snapshot| {
            let address = addresses.get(snapshot.output().port().as_str())?.clone();
            let RuntimeValue::Relation(relation) = snapshot.value().value().unannotated() else {
                return None;
            };
            if relation.schema_is_deferred() {
                return None;
            }
            let fields = relation
                .schema()
                .fields()
                .iter()
                .map(|field| SchemaField {
                    name: SchemaColumnRef(field.name().as_str().into()),
                    scalar_type: yss_database_arrow::column_semantic(field)
                        .map(|semantic| RelationalScalarType::Known(semantic.kind))
                        .unwrap_or(RelationalScalarType::Unknown),
                    lineage: None,
                })
                .collect();
            Some((
                address,
                GraphSchemaObservation {
                    version: snapshot.provenance().result_id().get(),
                    fields,
                },
            ))
        })
        .collect()
}
