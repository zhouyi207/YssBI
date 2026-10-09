use super::*;
use crate::binding_cleanup::BindingCleanup;
use std::collections::BTreeSet;
use yss_graph_document_edit::GraphDocumentPatchPreview;

pub(super) struct EditorMutationPlanner<'a> {
    graph_path: &'a GraphResourcePath,
    registry: &'a NodeRegistry,
    catalog: &'a CatalogMutationValidationSnapshot,
    semantics: Option<&'a GraphSemanticSnapshot>,
    candidate: GraphDocumentPatchPreview<'a>,
    cleanup: BindingCleanup<'a>,
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
            candidate: GraphDocumentPatchPreview::new(document),
            cleanup: BindingCleanup::new(document),
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
                self.candidate.read(),
                self.registry,
                context,
            )?;
            let staged = self.candidate.prepare(&patch)?;
            let cleanup = self.cleanup.operations(staged.document(), &patch);
            drop(staged);
            let mut operations = patch.operations;
            operations.extend(cleanup);
            operations
        } else {
            let mut claimed = self.candidate.prepare(&claims)?;
            let patch = mutation.into_patch_with_context(
                self.graph_path,
                claimed.read(),
                self.registry,
                context,
            )?;
            let staged = claimed.prepare(&patch)?;
            let cleanup = self.cleanup.operations(staged.document(), &patch);
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
        for address in mutation.referenced_ports(self.candidate.document()) {
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
            if let Some(previous) = self.candidate.document().port_bindings.get(address) {
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
