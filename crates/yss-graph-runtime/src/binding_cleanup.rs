use yss_graph_document::GraphDocumentOperation;
use yss_graph_document_edit::GraphDocumentRead;

/// Read only the unused derived bindings in the validated current candidate.
pub(super) fn operations(candidate: GraphDocumentRead<'_>) -> Vec<GraphDocumentOperation> {
    candidate
        .unreferenced_derived_bindings()
        .map(
            |(address, binding)| GraphDocumentOperation::RemovePortBinding {
                address: address.clone(),
                binding: binding.clone(),
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use yss_graph_document::{
        ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, FunctionParameterId,
        InputState, LastKnownPortMetadata, NodeId, NodePosition, OrderKey, ParameterValues,
        PortInstanceId,
    };
    use yss_graph_document::{
        DynamicPortBinding, GraphDocument, GraphDocumentOperation, GraphDocumentPatch, PortAddress,
    };
    use yss_graph_document_edit::GraphDocumentPatchPreview;

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
        let mut candidate = GraphDocumentPatchPreview::new(&document);
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
            let staged = candidate.prepare(&patch).unwrap();
            let cleanup = super::operations(staged.read());
            assert_eq!(
                cleanup,
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
            assert_eq!(candidate.document(), &document);
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
        let mut candidate = GraphDocumentPatchPreview::new(&document);
        let patch = GraphDocumentPatch::new(insert.clone());
        let staged = candidate.prepare(&patch).unwrap();
        assert!(super::operations(staged.read()).is_empty());
        drop(staged);

        let mut insert_then_remove = insert;
        insert_then_remove.push(GraphDocumentOperation::RemoveConnection { connection });
        let patch = GraphDocumentPatch::new(insert_then_remove);
        let staged = candidate.prepare(&patch).unwrap();
        assert_eq!(
            super::operations(staged.read()),
            vec![GraphDocumentOperation::RemovePortBinding { address, binding }]
        );
        drop(staged);
        assert_eq!(candidate.document(), &document);
    }
}
