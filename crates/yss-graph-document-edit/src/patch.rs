use crate::{DocumentError, validate_graph_document};
use yss_graph_document::{
    DocumentConnection, DocumentNode, DynamicPortBinding, GraphDocument, GraphDocumentOperation,
    GraphDocumentPatch, InputState, PortAddress,
};

fn apply_operation(
    operation: &GraphDocumentOperation,
    document: &mut GraphDocument,
) -> Result<(), DocumentError> {
    // A rejected operation must not write: scoped rollback records successful operations only.
    match operation {
        GraphDocumentOperation::SetConstant { id, before, after } => {
            if document.constants.get(id) != before.as_deref() {
                return Err(DocumentError::ConstantContentMismatch(*id));
            }
            if let Some(constant) = after {
                if constant.id != *id {
                    return Err(DocumentError::InvalidConstant(*id));
                }
                document.constants.insert(*id, constant.as_ref().clone());
            } else {
                document.constants.remove(id);
            }
            Ok(())
        }
        GraphDocumentOperation::InsertNode { node } => insert_node(document, node),
        GraphDocumentOperation::RemoveNode { node } => remove_node(document, node),
        GraphDocumentOperation::UpdateNode { before, after } => {
            update_node(document, before, after)
        }
        GraphDocumentOperation::InsertPortBinding { address, binding } => {
            insert_port_binding(document, address, binding)
        }
        GraphDocumentOperation::RemovePortBinding { address, binding } => {
            remove_port_binding(document, address, binding)
        }
        GraphDocumentOperation::InsertConnection { connection } => {
            insert_connection(document, connection)
        }
        GraphDocumentOperation::RemoveConnection { connection } => {
            remove_connection(document, connection)
        }
        GraphDocumentOperation::SetInputState {
            address,
            before,
            after,
        } => set_input_state(document, address, before, after),
    }
}

fn insert_node(document: &mut GraphDocument, node: &DocumentNode) -> Result<(), DocumentError> {
    if document.nodes.contains_key(&node.id) {
        return Err(DocumentError::DuplicateNode(node.id));
    }
    document.nodes.insert(node.id, node.clone());
    Ok(())
}

fn remove_node(document: &mut GraphDocument, node: &DocumentNode) -> Result<(), DocumentError> {
    match document.nodes.get(&node.id) {
        None => Err(DocumentError::NodeNotFound(node.id)),
        Some(current) if current != node => Err(DocumentError::NodeContentMismatch(node.id)),
        Some(_) => {
            document.nodes.remove(&node.id);
            Ok(())
        }
    }
}

fn update_node(
    document: &mut GraphDocument,
    before: &DocumentNode,
    after: &DocumentNode,
) -> Result<(), DocumentError> {
    if before.id != after.id {
        return Err(DocumentError::NodeIdentityMismatch {
            before: before.id,
            after: after.id,
        });
    }
    match document.nodes.get(&before.id) {
        None => Err(DocumentError::NodeNotFound(before.id)),
        Some(current) if current != before => Err(DocumentError::NodeContentMismatch(before.id)),
        Some(_) => {
            document.nodes.insert(after.id, after.clone());
            Ok(())
        }
    }
}

fn insert_port_binding(
    document: &mut GraphDocument,
    address: &PortAddress,
    binding: &DynamicPortBinding,
) -> Result<(), DocumentError> {
    if document.port_bindings.contains_key(address) {
        return Err(DocumentError::DuplicatePortBinding(address.clone()));
    }
    document
        .port_bindings
        .insert(address.clone(), binding.clone());
    Ok(())
}

fn remove_port_binding(
    document: &mut GraphDocument,
    address: &PortAddress,
    binding: &DynamicPortBinding,
) -> Result<(), DocumentError> {
    match document.port_bindings.get(address) {
        None => Err(DocumentError::PortBindingNotFound(address.clone())),
        Some(current) if current != binding => {
            Err(DocumentError::PortBindingContentMismatch(address.clone()))
        }
        Some(_) => {
            document.port_bindings.remove(address);
            Ok(())
        }
    }
}

fn insert_connection(
    document: &mut GraphDocument,
    connection: &DocumentConnection,
) -> Result<(), DocumentError> {
    if document.connections.contains_key(&connection.id) {
        return Err(DocumentError::DuplicateConnection(connection.id));
    }
    document
        .connections
        .insert(connection.id, connection.clone());
    Ok(())
}

fn remove_connection(
    document: &mut GraphDocument,
    connection: &DocumentConnection,
) -> Result<(), DocumentError> {
    validate_connection_removal(document, connection)?;
    document.connections.remove(&connection.id);
    Ok(())
}

pub(crate) fn validate_connection_removal(
    document: &GraphDocument,
    connection: &DocumentConnection,
) -> Result<(), DocumentError> {
    match document.connections.get(&connection.id) {
        None => Err(DocumentError::ConnectionNotFound(connection.id)),
        Some(current) if current != connection => {
            Err(DocumentError::ConnectionContentMismatch(connection.id))
        }
        Some(_) => Ok(()),
    }
}

fn set_input_state(
    document: &mut GraphDocument,
    address: &PortAddress,
    before: &Option<InputState>,
    after: &Option<InputState>,
) -> Result<(), DocumentError> {
    if document.input_states.get(address) != before.as_ref() {
        return Err(DocumentError::InputStateMismatch(address.clone()));
    }
    match after {
        Some(state) => {
            document.input_states.insert(address.clone(), state.clone());
        }
        None => {
            document.input_states.remove(address);
        }
    }
    Ok(())
}

/// Consume a private candidate; callers fork shared source documents before preparing edits.
pub fn prepare_graph_document_patch(
    mut document: GraphDocument,
    patch: &GraphDocumentPatch,
) -> Result<GraphDocument, DocumentError> {
    apply_graph_document_patch(&mut document, patch)?;
    Ok(document)
}

/// A validated temporary patch whose changes are restored when the scope ends.
#[must_use = "the staged patch is restored when its guard is dropped"]
pub struct PreparedGraphDocumentPatch<'a> {
    document: &'a mut GraphDocument,
    operations: &'a [GraphDocumentOperation],
    applied: usize,
}

impl PreparedGraphDocumentPatch<'_> {
    pub fn document(&self) -> &GraphDocument {
        self.document
    }

    pub fn prepare<'stage>(
        &'stage mut self,
        patch: &'stage GraphDocumentPatch,
    ) -> Result<PreparedGraphDocumentPatch<'stage>, DocumentError> {
        prepare_graph_document_patch_in_place(self.document, patch)
    }

    fn commit(mut self) {
        self.applied = 0;
    }
}

impl Drop for PreparedGraphDocumentPatch<'_> {
    fn drop(&mut self) {
        for operation in self.operations[..self.applied].iter().rev() {
            restore_operation(operation, self.document);
        }
    }
}

/// Prepare a read-only temporary candidate without copying unrelated document content.
pub fn prepare_graph_document_patch_in_place<'a>(
    document: &'a mut GraphDocument,
    patch: &'a GraphDocumentPatch,
) -> Result<PreparedGraphDocumentPatch<'a>, DocumentError> {
    let mut candidate = PreparedGraphDocumentPatch {
        document,
        operations: &patch.operations,
        applied: 0,
    };
    for operation in &patch.operations {
        apply_operation(operation, candidate.document)?;
        candidate.applied += 1;
    }
    validate_graph_document(candidate.document)?;
    Ok(candidate)
}

pub fn apply_graph_document_patch(
    document: &mut GraphDocument,
    patch: &GraphDocumentPatch,
) -> Result<(), DocumentError> {
    prepare_graph_document_patch_in_place(document, patch)?.commit();
    Ok(())
}

fn restore_operation(operation: &GraphDocumentOperation, document: &mut GraphDocument) {
    match operation {
        GraphDocumentOperation::SetConstant { id, before, .. } => {
            if let Some(before) = before {
                document.constants.insert(*id, before.as_ref().clone());
            } else {
                document.constants.remove(id);
            }
        }
        GraphDocumentOperation::InsertNode { node } => {
            document.nodes.remove(&node.id);
        }
        GraphDocumentOperation::RemoveNode { node }
        | GraphDocumentOperation::UpdateNode { before: node, .. } => {
            document.nodes.insert(node.id, node.clone());
        }
        GraphDocumentOperation::InsertPortBinding { address, .. } => {
            document.port_bindings.remove(address);
        }
        GraphDocumentOperation::RemovePortBinding { address, binding } => {
            document
                .port_bindings
                .insert(address.clone(), binding.clone());
        }
        GraphDocumentOperation::InsertConnection { connection } => {
            document.connections.remove(&connection.id);
        }
        GraphDocumentOperation::RemoveConnection { connection } => {
            document
                .connections
                .insert(connection.id, connection.clone());
        }
        GraphDocumentOperation::SetInputState {
            address, before, ..
        } => {
            if let Some(before) = before {
                document
                    .input_states
                    .insert(address.clone(), before.clone());
            } else {
                document.input_states.remove(address);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DocumentError, GraphDocumentOperation, GraphDocumentPatch, apply_graph_document_patch,
        prepare_graph_document_patch, prepare_graph_document_patch_in_place,
    };
    use yss_graph_document::{
        DocumentNode, DynamicPortBinding, GraphDocument, NodeId, NodePosition, OrderKey,
        ParameterValues, PortAddress,
    };
    use yss_node_protocol::{NodeTypeId, PortKey};

    #[test]
    fn prepared_patch_keeps_source_and_checks_sequential_before_states() {
        let id = NodeId::new();
        let before = DocumentNode {
            id,
            node_type: NodeTypeId::new("yssbi.test.patch").unwrap(),
            position: NodePosition { x: 1.0, y: 2.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        };
        let mut after = before.clone();
        after.position.x = 42.0;
        let mut document = GraphDocument::default();
        document.nodes.insert(id, before.clone());
        let update = GraphDocumentOperation::UpdateNode {
            before: before.clone(),
            after: after.clone(),
        };
        let prepared = prepare_graph_document_patch(
            document.clone(),
            &GraphDocumentPatch::new([update.clone()]),
        )
        .unwrap();
        assert_eq!(document.nodes[&id], before);
        assert_eq!(prepared.nodes[&id], after);

        let failed = prepare_graph_document_patch(
            document.clone(),
            &GraphDocumentPatch::new([
                update,
                GraphDocumentOperation::RemoveNode {
                    node: before.clone(),
                },
            ]),
        );
        assert_eq!(failed.unwrap_err(), DocumentError::NodeContentMismatch(id));
        assert_eq!(document.nodes[&id], before);
    }

    #[test]
    fn patch_commit_is_atomic() {
        let node_id = NodeId::new();
        let node = DocumentNode {
            id: node_id,
            node_type: NodeTypeId::new("yssbi.test.patch").unwrap(),
            position: NodePosition { x: 1.0, y: 2.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        };
        let mut document = GraphDocument::default();

        apply_graph_document_patch(
            &mut document,
            &GraphDocumentPatch::new([GraphDocumentOperation::InsertNode { node }]),
        )
        .unwrap();

        assert!(document.nodes.contains_key(&node_id));

        let before_invalid_patch = document.clone();
        let declared_port = PortAddress::declared(node_id, PortKey::new("input").unwrap());
        let error = apply_graph_document_patch(
            &mut document,
            &GraphDocumentPatch::new([GraphDocumentOperation::InsertPortBinding {
                address: declared_port.clone(),
                binding: DynamicPortBinding::UserCreated {
                    order: OrderKey::new("rank-a"),
                },
            }]),
        )
        .unwrap_err();

        assert_eq!(error, DocumentError::UnexpectedPortBinding(declared_port));
        assert_eq!(document, before_invalid_patch);
    }

    #[test]
    fn nested_patch_scopes_restore_the_parent_after_success_and_conflict() {
        let id = NodeId::new();
        let before = DocumentNode {
            id,
            node_type: NodeTypeId::new("yssbi.test.patch").unwrap(),
            position: NodePosition { x: 1.0, y: 2.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        };
        let mut outer = before.clone();
        outer.user_label = Some("outer".into());
        let mut inner = outer.clone();
        inner.user_label = Some("inner".into());
        let outer_patch = GraphDocumentPatch::new([GraphDocumentOperation::UpdateNode {
            before: before.clone(),
            after: outer.clone(),
        }]);
        let inner_operation = GraphDocumentOperation::UpdateNode {
            before: outer.clone(),
            after: inner.clone(),
        };
        let inner_patch = GraphDocumentPatch::new([inner_operation.clone()]);
        let rejected = GraphDocumentPatch::new([
            inner_operation,
            GraphDocumentOperation::RemoveNode {
                node: before.clone(),
            },
        ]);
        let mut document = GraphDocument::default();
        document.nodes.insert(id, before.clone());
        let mut prepared =
            prepare_graph_document_patch_in_place(&mut document, &outer_patch).unwrap();
        assert_eq!(prepared.document().nodes[&id], outer);
        {
            let nested = prepared.prepare(&inner_patch).unwrap();
            assert_eq!(nested.document().nodes[&id], inner);
        }
        assert_eq!(prepared.document().nodes[&id], outer);
        assert!(
            matches!(prepared.prepare(&rejected), Err(DocumentError::NodeContentMismatch(node)) if node == id)
        );
        assert_eq!(prepared.document().nodes[&id], outer);
        drop(prepared);
        assert_eq!(document.nodes[&id], before);
    }

    #[test]
    fn failed_validation_restores_non_reflexive_intermediate_values() {
        let id = NodeId::new();
        let before = DocumentNode {
            id,
            node_type: NodeTypeId::new("yssbi.test.patch").unwrap(),
            position: NodePosition { x: 1.0, y: 2.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        };
        let mut after = before.clone();
        after.position.x = f64::NAN;
        let address = PortAddress::declared(id, PortKey::new("input").unwrap());
        let patch = GraphDocumentPatch::new([
            GraphDocumentOperation::UpdateNode {
                before: before.clone(),
                after,
            },
            GraphDocumentOperation::InsertPortBinding {
                address: address.clone(),
                binding: DynamicPortBinding::UserCreated {
                    order: OrderKey::new("a"),
                },
            },
        ]);
        let mut document = GraphDocument::default();
        document.nodes.insert(id, before);
        let original = document.clone();
        assert_eq!(
            apply_graph_document_patch(&mut document, &patch),
            Err(DocumentError::UnexpectedPortBinding(address))
        );
        assert_eq!(document, original);
    }

    #[test]
    fn borrowed_connection_candidates_preserve_removal_preconditions_and_result_validation() {
        use crate::validate_graph_document_connection_candidate;
        use std::collections::BTreeMap;
        use yss_graph_document::{ConnectionId, DocumentConnection};
        let mut document = GraphDocument::default();
        let nodes = [NodeId::new(), NodeId::new()];
        for id in nodes {
            document.nodes.insert(
                id,
                DocumentNode {
                    id,
                    node_type: NodeTypeId::new("yssbi.test.patch").unwrap(),
                    position: NodePosition { x: 0.0, y: 0.0 },
                    parameters: ParameterValues::new(),
                    user_label: None,
                },
            );
        }
        let id = ConnectionId::new();
        let original = yss_graph_document::DocumentConnection {
            id,
            output: PortAddress::declared(nodes[0], PortKey::new("output").unwrap()),
            input: PortAddress::declared(NodeId::new(), PortKey::new("input").unwrap()),
            order: None,
        };
        document.connections.insert(id, original.clone());
        let replacement = DocumentConnection {
            input: PortAddress::declared(nodes[1], PortKey::new("input").unwrap()),
            ..original.clone()
        };
        let removals = BTreeMap::from([(id, original.clone())]);
        let insertions = BTreeMap::from([(id, replacement.clone())]);
        validate_graph_document_connection_candidate(&document, &removals, &insertions).unwrap();
        let patch = GraphDocumentPatch::new([
            GraphDocumentOperation::RemoveConnection {
                connection: original.clone(),
            },
            GraphDocumentOperation::InsertConnection {
                connection: replacement.clone(),
            },
        ]);
        let prepared = prepare_graph_document_patch(document.clone(), &patch).unwrap();
        assert_eq!(prepared.connections[&id], replacement);
        assert_eq!(document.connections[&id], original);

        let wrong_before = BTreeMap::from([(id, replacement.clone())]);
        assert_eq!(
            validate_graph_document_connection_candidate(&document, &wrong_before, &insertions),
            Err(DocumentError::ConnectionContentMismatch(id))
        );
        assert_eq!(
            validate_graph_document_connection_candidate(&document, &BTreeMap::new(), &insertions),
            Err(DocumentError::DuplicateConnection(id))
        );
        let invalid_after = BTreeMap::from([(id, original.clone())]);
        let missing = original.input.node_id;
        assert_eq!(
            validate_graph_document_connection_candidate(&document, &removals, &invalid_after),
            Err(DocumentError::EndpointNodeNotFound(missing))
        );
    }
}
