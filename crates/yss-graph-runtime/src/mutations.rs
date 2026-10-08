use super::*;
use std::borrow::Cow;
use std::collections::BTreeSet;
use yss_graph_document_edit::prepare_graph_document_patch_in_place;

pub(super) struct EditorMutationPlanner<'a> {
    graph_path: &'a GraphResourcePath,
    registry: &'a NodeRegistry,
    catalog: &'a CatalogMutationValidationSnapshot,
    semantics: Option<&'a GraphSemanticSnapshot>,
    candidate: Cow<'a, GraphDocument>,
}

impl<'a> EditorMutationPlanner<'a> {
    pub(super) fn new(
        graph_path: &'a GraphResourcePath,
        document: &'a GraphDocument,
        registry: &'a NodeRegistry,
        catalog: &'a CatalogMutationValidationSnapshot,
        analysis: Option<&'a GraphAnalysis>,
    ) -> Self {
        Self {
            graph_path,
            registry,
            catalog,
            semantics: analysis.map(GraphAnalysis::semantic_snapshot),
            candidate: Cow::Borrowed(document),
        }
    }

    pub(super) fn plan(
        &mut self,
        mutation: EditorGraphMutation,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        let claims = GraphDocumentPatch::new(self.claim_operations(&mutation)?);
        let context = EditorMutationContext {
            catalog: Some(self.catalog),
            semantics: self.semantics,
        };
        let operations = if claims.operations.is_empty() {
            let patch = mutation.into_patch_with_context(
                self.graph_path,
                &self.candidate,
                self.registry,
                context,
            )?;
            let staged = prepare_graph_document_patch_in_place(self.candidate.to_mut(), &patch)?;
            let cleanup = unused_binding_operations(staged.document());
            drop(staged);
            let mut operations = patch.operations;
            operations.extend(cleanup);
            operations
        } else {
            let mut claimed =
                prepare_graph_document_patch_in_place(self.candidate.to_mut(), &claims)?;
            let patch = mutation.into_patch_with_context(
                self.graph_path,
                claimed.document(),
                self.registry,
                context,
            )?;
            let staged = claimed.prepare(&patch)?;
            let cleanup = unused_binding_operations(staged.document());
            drop(staged);
            drop(claimed);
            let mut operations = claims.operations;
            operations.extend(patch.operations);
            operations.extend(cleanup);
            operations
        };
        Ok(GraphDocumentPatch::new(operations))
    }

    fn claim_operations(
        &self,
        mutation: &EditorGraphMutation,
    ) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
        let mut operations = Vec::new();
        let mut claimed = BTreeSet::new();
        for address in mutation.referenced_ports(&self.candidate) {
            if !claimed.insert(address) {
                continue;
            }
            let semantics = self.semantics.expect("referenced ports require analysis");
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
            if let Some(previous) = self.candidate.port_bindings.get(address) {
                if let DynamicPortBinding::Orphan { origin, order, .. } = previous {
                    operations.push(GraphDocumentOperation::RemovePortBinding {
                        address: address.clone(),
                        binding: previous.clone(),
                    });
                    operations.push(GraphDocumentOperation::InsertPortBinding {
                        address: address.clone(),
                        binding: DynamicPortBinding::Resolved {
                            origin: origin.clone(),
                            order: order.clone(),
                            last_known: LastKnownPortMetadata {
                                label: port.label.to_string(),
                                value_type: Some(port.accepted_type.clone()),
                            },
                        },
                    });
                }
                continue;
            }
            let yss_graph_analysis::GraphPortBacking::ProjectedDerived { origin } = &port.backing
            else {
                continue;
            };
            operations.push(GraphDocumentOperation::InsertPortBinding {
                address: address.clone(),
                binding: DynamicPortBinding::Resolved {
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
                },
            });
        }
        Ok(operations)
    }
}

fn unused_binding_operations(candidate: &GraphDocument) -> Vec<GraphDocumentOperation> {
    if candidate
        .port_bindings
        .values()
        .all(|binding| matches!(binding, DynamicPortBinding::UserCreated { .. }))
    {
        return Vec::new();
    }
    let referenced_ports = candidate
        .connections
        .values()
        .flat_map(|connection| [&connection.output, &connection.input])
        .chain(candidate.input_states.keys())
        .collect::<BTreeSet<_>>();
    candidate
        .port_bindings
        .iter()
        .filter(|(address, binding)| {
            !matches!(binding, DynamicPortBinding::UserCreated { .. })
                && !referenced_ports.contains(address)
        })
        .map(
            |(address, binding)| GraphDocumentOperation::RemovePortBinding {
                address: address.clone(),
                binding: binding.clone(),
            },
        )
        .collect()
}
