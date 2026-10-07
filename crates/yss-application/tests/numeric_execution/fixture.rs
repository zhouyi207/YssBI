//! Graph construction and result inspection shared by statistical integration fixtures.
use super::*;
use serde_json::{Value, json};
use yss_data_contract::{DataValue, ValueType};
use yss_graph_document::{DynamicPortBinding, OrderKey, PortInstanceId};

pub(super) fn node(document: &mut GraphDocument, kind: &str, parameters: Value) -> NodeId {
    let id = NodeId::new();
    document.nodes.insert(
        id,
        DocumentNode {
            id,
            node_type: kind.parse().unwrap(),
            position: NodePosition { x: 0., y: 0. },
            parameters: serde_json::from_value(parameters).unwrap(),
            user_label: None,
        },
    );
    id
}
pub(super) fn connect(
    document: &mut GraphDocument,
    from: NodeId,
    output: &str,
    to: NodeId,
    input: &str,
    repeat: Option<usize>,
) {
    let input = if let Some(order) = repeat {
        let address = PortAddress::instance(to, input.parse().unwrap(), PortInstanceId::new());
        document.port_bindings.insert(
            address.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(format!("{order:03}")),
            },
        );
        address
    } else {
        PortAddress::declared(to, input.parse().unwrap())
    };
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(from, output.parse().unwrap()),
            input,
            order: None,
        },
    );
}
pub(super) fn columns(
    document: &mut GraphDocument,
    values: &[(&str, Value)],
) -> BTreeMap<String, NodeId> {
    let source = node(document, "yssbi.constant.get", json!({}));
    let table = Value::Object(
        values
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect(),
    );
    set_constant(
        document,
        source,
        ValueType::DataFrame,
        DataValue::String(table.to_string().into()),
    );
    for constant in document.constants.values_mut() {
        yss_graph_document::normalize_constant_value(constant).unwrap();
    }
    values
        .iter()
        .map(|(key, _)| {
            let select = node(
                document,
                "yssbi.dataframe.series.select",
                json!({"column":key}),
            );
            connect(document, source, "value", select, "dataframe", None);
            ((*key).to_string(), select)
        })
        .collect()
}
pub(super) fn field<'a>(value: &'a RuntimeValue, key: &str) -> &'a RuntimeValue {
    let RuntimeValue::Record(fields) = value else {
        panic!("expected record: {value:?}")
    };
    fields.get(key).unwrap_or_else(|| panic!("missing {key}"))
}
pub(super) fn number(value: &RuntimeValue) -> f64 {
    let RuntimeValue::Scalar(value) = value else {
        panic!("expected scalar")
    };
    serde_json::to_value(value).unwrap().as_f64().unwrap()
}
