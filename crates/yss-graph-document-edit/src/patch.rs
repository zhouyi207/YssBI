use crate::{DocumentError, validate_graph_document};
use yss_graph_document::{
    DocumentConnection, DocumentNode, DynamicPortBinding, GraphDocument, GraphDocumentOperation,
    GraphDocumentPatch, InputState, PortAddress,
};

fn apply_operation(
    operation: &GraphDocumentOperation,
    document: &mut GraphDocument,
) -> Result<(), DocumentError> {
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
    match document.connections.get(&connection.id) {
        None => Err(DocumentError::ConnectionNotFound(connection.id)),
        Some(current) if current != connection => {
            Err(DocumentError::ConnectionContentMismatch(connection.id))
        }
        Some(_) => {
            document.connections.remove(&connection.id);
            Ok(())
        }
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

pub fn apply_graph_document_patch(
    document: &mut GraphDocument,
    patch: &GraphDocumentPatch,
) -> Result<(), DocumentError> {
    let mut staged = document.clone();
    for operation in &patch.operations {
        apply_operation(operation, &mut staged)?;
    }
    validate_graph_document(&staged)?;
    *document = staged;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DocumentError, GraphDocumentOperation, GraphDocumentPatch, apply_graph_document_patch,
    };
    use yss_graph_document::{
        DocumentNode, DynamicPortBinding, GraphDocument, NodeId, NodePosition, OrderKey,
        ParameterValues, PortAddress,
    };
    use yss_node_protocol::{NodeTypeId, PortKey};

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
}
