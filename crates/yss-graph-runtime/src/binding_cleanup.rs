use std::cell::OnceCell;
use std::collections::BTreeMap;
use yss_graph_document::{
    DynamicPortBinding, GraphDocument, GraphDocumentOperation, GraphDocumentPatch, PortAddress,
};

pub(super) struct BindingCleanup<'a> {
    document: &'a GraphDocument,
    references: OnceCell<BTreeMap<&'a PortAddress, usize>>,
}

impl<'a> BindingCleanup<'a> {
    pub(super) fn new(document: &'a GraphDocument) -> Self {
        Self {
            document,
            references: OnceCell::new(),
        }
    }

    /// Read a structurally validated candidate prepared from the original document.
    /// Port claims only change bindings; reference changes come from this edit patch.
    pub(super) fn operations(
        &self,
        candidate: &GraphDocument,
        patch: &GraphDocumentPatch,
    ) -> Vec<GraphDocumentOperation> {
        let mut bindings = candidate
            .port_bindings
            .iter()
            .filter(|(_, binding)| !matches!(binding, DynamicPortBinding::UserCreated { .. }))
            .peekable();
        if bindings.peek().is_none() {
            return Vec::new();
        }
        let references = self.references.get_or_init(|| {
            let mut references = BTreeMap::new();
            for address in self
                .document
                .connections
                .values()
                .flat_map(|connection| [&connection.output, &connection.input])
                .chain(self.document.input_states.keys())
                .filter(|address| address.is_instance())
            {
                *references.entry(address).or_insert(0) += 1;
            }
            references
        });
        let changes = reference_changes(patch);
        bindings
            .filter(|(address, _)| {
                let count = references.get(address).copied().unwrap_or(0);
                let (added, removed) = changes.get(address).copied().unwrap_or_default();
                count + added == removed
            })
            .map(
                |(address, binding)| GraphDocumentOperation::RemovePortBinding {
                    address: address.clone(),
                    binding: binding.clone(),
                },
            )
            .collect()
    }
}

fn reference_changes<'a>(
    patch: &'a GraphDocumentPatch,
) -> BTreeMap<&'a PortAddress, (usize, usize)> {
    let mut changes = BTreeMap::<_, (usize, usize)>::new();
    let mut record = |address: &'a PortAddress, added| {
        if address.is_instance() {
            let change = changes.entry(address).or_default();
            if added {
                change.0 += 1;
            } else {
                change.1 += 1;
            }
        }
    };
    for operation in &patch.operations {
        match operation {
            GraphDocumentOperation::InsertConnection { connection } => {
                record(&connection.output, true);
                record(&connection.input, true);
            }
            GraphDocumentOperation::RemoveConnection { connection } => {
                record(&connection.output, false);
                record(&connection.input, false);
            }
            GraphDocumentOperation::SetInputState {
                address,
                before,
                after,
            } => match (before, after) {
                (None, Some(_)) => record(address, true),
                (Some(_), None) => record(address, false),
                _ => {}
            },
            _ => {}
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_graph_document::{
        ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, FunctionParameterId,
        InputState, LastKnownPortMetadata, NodeId, NodePosition, OrderKey, ParameterValues,
        PortInstanceId,
    };
    use yss_graph_document_edit::prepare_graph_document_patch_in_place;

    fn fixture() -> (
        GraphDocument,
        PortAddress,
        DynamicPortBinding,
        DocumentConnection,
    ) {
        let mut document = GraphDocument::default();
        for id in [1u128, 2] {
            let id = NodeId::from_bytes(id.to_be_bytes());
            document.nodes.insert(
                id,
                DocumentNode {
                    id,
                    node_type: "tests.cleanup.node".parse().unwrap(),
                    position: NodePosition { x: 0.0, y: 0.0 },
                    parameters: ParameterValues::new(),
                    user_label: None,
                },
            );
        }
        let address = PortAddress::instance(
            NodeId::from_bytes(2u128.to_be_bytes()),
            "arguments".parse().unwrap(),
            PortInstanceId::from_bytes(1u128.to_be_bytes()),
        );
        let binding = DynamicPortBinding::Resolved {
            origin: DynamicMemberLocator::FunctionParameter {
                function: "functions/Flag.yssbi-function".parse().unwrap(),
                parameter: FunctionParameterId::new("flag"),
            },
            order: OrderKey::new("0"),
            last_known: LastKnownPortMetadata::default(),
        };
        let connection = DocumentConnection {
            id: ConnectionId::from_bytes(1u128.to_be_bytes()),
            output: PortAddress::declared(
                NodeId::from_bytes(1u128.to_be_bytes()),
                "result".parse().unwrap(),
            ),
            input: address.clone(),
            order: None,
        };
        (document, address, binding, connection)
    }

    #[test]
    fn cleanup_keeps_shared_connection_and_input_references_for_independent_candidates() {
        let (mut document, address, binding, first) = fixture();
        document
            .port_bindings
            .insert(address.clone(), binding.clone());
        let mut second = first.clone();
        second.id = ConnectionId::from_bytes(2u128.to_be_bytes());
        second.output = PortAddress::declared(first.output.node_id, "other".parse().unwrap());
        document.connections.insert(first.id, first.clone());
        document.connections.insert(second.id, second.clone());
        let state = InputState {
            literal_override: None,
        };
        document.input_states.insert(address.clone(), state.clone());
        let remove_first = GraphDocumentOperation::RemoveConnection { connection: first };
        let remove_second = GraphDocumentOperation::RemoveConnection { connection: second };
        let remove_state = GraphDocumentOperation::SetInputState {
            address: address.clone(),
            before: Some(state),
            after: None,
        };
        let cleanup = BindingCleanup::new(&document);
        let mut candidate = document.clone();
        // Removing all connections still leaves the input state; only the third
        // candidate has no references. The last candidate starts from the original.
        for (operations, unused) in [
            (vec![remove_first.clone()], false),
            (vec![remove_first.clone(), remove_second.clone()], false),
            (
                vec![remove_first, remove_second, remove_state.clone()],
                true,
            ),
            (vec![remove_state], false),
        ] {
            let patch = GraphDocumentPatch::new(operations);
            let staged = prepare_graph_document_patch_in_place(&mut candidate, &patch).unwrap();
            let operations = cleanup.operations(staged.document(), &patch);
            assert_eq!(
                operations,
                if unused {
                    vec![GraphDocumentOperation::RemovePortBinding {
                        address: address.clone(),
                        binding: binding.clone(),
                    }]
                } else {
                    Vec::new()
                }
            );
            drop(staged);
            assert_eq!(candidate, document);
        }
    }

    #[test]
    fn cleanup_accounts_for_new_claims_and_never_removes_user_created_bindings() {
        let (document, address, binding, connection) = fixture();
        let user = PortAddress::instance(
            address.node_id,
            "user".parse().unwrap(),
            PortInstanceId::from_bytes(2u128.to_be_bytes()),
        );
        let insert = vec![
            GraphDocumentOperation::InsertPortBinding {
                address: address.clone(),
                binding: binding.clone(),
            },
            GraphDocumentOperation::InsertPortBinding {
                address: user,
                binding: DynamicPortBinding::UserCreated {
                    order: OrderKey::new("0"),
                },
            },
            GraphDocumentOperation::InsertConnection {
                connection: connection.clone(),
            },
        ];
        let cleanup = BindingCleanup::new(&document);
        let mut candidate = document.clone();
        let patch = GraphDocumentPatch::new(insert.clone());
        let staged = prepare_graph_document_patch_in_place(&mut candidate, &patch).unwrap();
        assert!(cleanup.operations(staged.document(), &patch).is_empty());
        drop(staged);

        let mut insert_then_remove = insert;
        insert_then_remove.push(GraphDocumentOperation::RemoveConnection { connection });
        let patch = GraphDocumentPatch::new(insert_then_remove);
        let staged = prepare_graph_document_patch_in_place(&mut candidate, &patch).unwrap();
        assert_eq!(
            cleanup.operations(staged.document(), &patch),
            vec![GraphDocumentOperation::RemovePortBinding { address, binding }]
        );
        drop(staged);
        assert_eq!(candidate, document);
    }
}
