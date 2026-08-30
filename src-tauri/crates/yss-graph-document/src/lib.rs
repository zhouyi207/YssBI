mod identity;
mod model;
mod resource_path;

pub use identity::{ConnectionId, GraphRevision, NodeId, PortInstanceId, RevisionExhausted};
pub use model::{
    DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionParameterId, GraphDocument, InputState, LastKnownPortMetadata, NodePosition, OrderKey,
    ParameterValues, PortAddress, PortRef, SchemaFieldIdentity, SchemaSourceIdentity, TypedValue,
};
pub use resource_path::{
    GraphResourceKind, GraphResourcePath, GraphResourcePathError, normalize_graph_resource_path,
};

#[cfg(test)]
mod tests {
    use super::{
        DocumentNode, GraphDocument, GraphResourcePath, GraphResourcePathError, NodeId,
        NodePosition, ParameterValues, TypedValue,
    };
    use serde_json::json;
    use yss_graph_protocol::{NodeTypeId, ParameterKey};

    #[test]
    fn typed_value_wire_remains_untagged_json() {
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(1));
        let literal: TypedValue = json!({
            "null": null,
            "bool": true,
            "number": 42.5,
            "string": "stable",
            "array": [null, false, 7, "nested"],
            "object": { "answer": 42 }
        });
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new("yssbi.test.document_wire").unwrap(),
                position: NodePosition { x: 1.0, y: 2.0 },
                parameters: ParameterValues::from([(
                    ParameterKey::new("literal").unwrap(),
                    literal.clone(),
                )]),
                user_label: None,
            },
        );

        let encoded = serde_json::to_value(&document).unwrap();
        assert_eq!(
            encoded,
            json!({
                "nodes": {
                    "00000000-0000-0000-0000-000000000001": {
                        "id": "00000000-0000-0000-0000-000000000001",
                        "node_type": "yssbi.test.document_wire",
                        "position": { "x": 1.0, "y": 2.0 },
                        "parameters": { "literal": literal },
                        "user_label": null
                    }
                },
                "port_bindings": [],
                "connections": {},
                "input_states": []
            })
        );
        assert_eq!(
            serde_json::from_value::<GraphDocument>(encoded).unwrap(),
            document
        );
    }

    #[test]
    fn graph_resource_names_preserve_exact_unicode_validation() {
        assert_eq!(
            GraphResourcePath::new("events/Re\u{301}sume\u{301}.yssbi-event"),
            Err(GraphResourcePathError::InvalidName)
        );
        assert_eq!(
            GraphResourcePath::new("events/Sales\u{0345}.yssbi-event"),
            Err(GraphResourcePathError::InvalidName)
        );
        assert!(GraphResourcePath::new("events/销售分析 2.yssbi-event").is_ok());
    }
}
