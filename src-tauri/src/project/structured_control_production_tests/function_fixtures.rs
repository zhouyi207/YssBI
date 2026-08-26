use super::{connection, declared, node, node_id};
use crate::graph_document::GraphResourcePath;
use crate::graph_document::{
    DynamicMemberLocator, DynamicPortBinding, FunctionParameterId, OrderKey, PortAddress,
    PortInstanceId,
};
use crate::node_system::document::{FunctionDocument, FunctionParameter, FunctionSignature};
use crate::node_system::protocol::{ParameterKey, PortKey};
use crate::project::{GraphDocumentKind, GraphResourceDocument};
use uuid::Uuid;

pub(super) const PARAMETER_ID: &str = "amount";
pub(super) const RETURN_ID: &str = "return";

pub(super) fn parameter_id() -> FunctionParameterId {
    FunctionParameterId::new(PARAMETER_ID)
}

pub(super) fn return_id() -> FunctionParameterId {
    FunctionParameterId::new(RETURN_ID)
}

pub(super) fn resolved_function_port(
    resource: &mut GraphResourceDocument,
    node: u128,
    template: &str,
    member: u128,
    function: &GraphResourcePath,
    parameter: &FunctionParameterId,
    order: &str,
) -> PortAddress {
    let address = PortAddress::instance(
        node_id(node),
        PortKey::new(template).unwrap(),
        PortInstanceId::from_uuid(Uuid::from_u128(member)),
    );
    assert!(
        resource
            .document
            .port_bindings
            .insert(
                address.clone(),
                DynamicPortBinding::Resolved {
                    origin: DynamicMemberLocator::FunctionParameter {
                        function: function.clone(),
                        parameter: parameter.clone(),
                    },
                    order: OrderKey::new(order),
                    last_known: crate::graph_document::LastKnownPortMetadata::default(),
                },
            )
            .is_none()
    );
    address
}

fn function_shell(path: &GraphResourcePath, name: &str) -> GraphResourceDocument {
    let mut resource = GraphResourceDocument::new(name, GraphDocumentKind::Function);
    resource.function = Some(FunctionDocument::new(FunctionSignature {
        parameters: vec![FunctionParameter {
            id: parameter_id(),
            name: "Amount".into(),
            type_name: "Int64".into(),
        }],
        return_type: Some("Int64".into()),
    }));
    for (id, node_type) in [
        (100, "yssbi.project.function.entry"),
        (400, "yssbi.project.function.return"),
    ] {
        let mut shell = node(id, node_type);
        shell.parameters.insert(
            ParameterKey::new("function").unwrap(),
            serde_json::json!(path.as_str()),
        );
        assert!(resource.document.nodes.insert(shell.id, shell).is_none());
    }
    resource
}

pub(super) struct UnaryFunctionFixture {
    pub(super) resource: GraphResourceDocument,
    pub(super) offset_node: crate::graph_document::NodeId,
}

pub(super) fn unary_add_function(
    path: &GraphResourcePath,
    name: &str,
    offset: i64,
) -> UnaryFunctionFixture {
    let mut resource = function_shell(path, name);
    let body = node(250, "yssbi.numeric.add.int64");
    let mut offset_node = node(10, "yssbi.constant.int64");
    offset_node.parameters.insert(
        ParameterKey::new("value").unwrap(),
        serde_json::json!(offset),
    );
    let entry_parameter = resolved_function_port(
        &mut resource,
        100,
        "parameters",
        9_001,
        path,
        &parameter_id(),
        "parameter-z",
    );
    let return_result = resolved_function_port(
        &mut resource,
        400,
        "results",
        9_002,
        path,
        &return_id(),
        "result-a",
    );
    for entry in [body.clone(), offset_node.clone()] {
        assert!(resource.document.nodes.insert(entry.id, entry).is_none());
    }
    for edge in [
        connection(90_001, declared(100, "then"), declared(400, "enter")),
        connection(90_002, entry_parameter, declared(250, "left")),
        connection(90_003, declared(10, "value"), declared(250, "right")),
        connection(90_004, declared(250, "result"), return_result),
    ] {
        assert!(
            resource
                .document
                .connections
                .insert(edge.id, edge)
                .is_none()
        );
    }
    UnaryFunctionFixture {
        resource,
        offset_node: offset_node.id,
    }
}
