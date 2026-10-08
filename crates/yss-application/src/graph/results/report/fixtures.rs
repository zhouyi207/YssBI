//! Shared real linear-model graph used by report query and addition regressions.
use super::LinearSummaryOptions;
use yss_data_contract::{DataValue, ValueType};
use yss_graph_document::{
    DocumentConnection, DocumentNode, GraphConstant, GraphDocument, GraphResourcePath, NodeId,
    NodePosition, ParameterValues, PortAddress,
};
use yss_graph_editor::{EditorGraphMutation, PortPlacement};

pub(super) fn linear_document(
    n: usize,
    method: &str,
    summary_options: LinearSummaryOptions,
) -> (GraphResourcePath, GraphDocument) {
    let graph = GraphResourcePath::new("events/report.yssbi-event").unwrap();
    let builtins = yss_node_catalog::build_builtin_node_system().unwrap();
    let mut document = GraphDocument::default();
    let [response, predictor, fit, summary] = std::array::from_fn(|_| NodeId::new());
    for (id, node_type) in [
        (response, "yssbi.constant.get"),
        (predictor, "yssbi.constant.get"),
        (fit, "yssbi.statistics.linear.fit"),
        (summary, "yssbi.statistics.linear.summary"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: node_type.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    {
        let options = &summary_options;
        let schema = &builtins
            .registry
            .protocol(&"yssbi.statistics.linear.summary".parse().unwrap())
            .unwrap()
            .parameters;
        let values = serde_json::to_value(options)
            .unwrap()
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.parse().unwrap(), value.clone()))
            .collect();
        document.nodes.get_mut(&summary).unwrap().parameters = schema
            .merge_values(&ParameterValues::new(), values)
            .unwrap();
    }
    for (node, values) in [
        (
            response,
            (0..n)
                .map(|i| 3.0 + 2.0 * i as f64 / n as f64 + (i % 7) as f64 * 0.01)
                .collect::<Vec<_>>(),
        ),
        (
            predictor,
            (0..n).map(|i| i as f64 / n as f64).collect::<Vec<_>>(),
        ),
    ] {
        let id = yss_graph_document::ConstantId::new();
        let mut constant = GraphConstant {
            id,
            name: node.to_string(),
            data_type: ValueType::DataSeries(Box::new(ValueType::Scalar(
                yss_data_contract::SemanticType::Numeric,
            ))),
            data_value: DataValue::String(
                (serde_json::json!({ "value": values }).to_string()).into(),
            ),
            tabular: None,
            description: String::new(),
            tags: vec![],
        };
        yss_graph_document::normalize_constant_value(&mut constant).unwrap();
        document.constants.insert(id, constant);
        document
            .nodes
            .get_mut(&node)
            .unwrap()
            .parameters
            .insert("constant".parse().unwrap(), id.to_string().into());
    }
    let patch = EditorGraphMutation::AddPortInstance {
        node_id: fit,
        template_key: "x".parse().unwrap(),
        placement: PortPlacement::Append,
    }
    .into_patch(&graph, &document, &builtins.registry)
    .unwrap();
    yss_graph_document_edit::apply_graph_document_patch(&mut document, &patch).unwrap();
    let input = document
        .port_bindings
        .keys()
        .find(|port| port.node_id == fit)
        .unwrap()
        .clone();
    for (source, input) in [
        (response, PortAddress::declared(fit, "y".parse().unwrap())),
        (predictor, input),
    ] {
        let id = yss_graph_document::ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(source, "value".parse().unwrap()),
                input,
                order: None,
            },
        );
    }
    let id = yss_graph_document::ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(fit, "model".parse().unwrap()),
            input: PortAddress::declared(summary, "model".parse().unwrap()),
            order: None,
        },
    );
    document.nodes.get_mut(&fit).unwrap().parameters =
        serde_json::from_value::<yss_node_protocol::ParameterValues>(
            serde_json::json!({"method": method, "constant": true, "covariance": "nonrobust"}),
        )
        .unwrap();
    let auxiliary = match method {
        "WLS" => vec![(0..n).map(|i| (i + 1) as f64).collect::<Vec<_>>()],
        "GLS" => (0..n)
            .map(|j| {
                (0..n)
                    .map(|i| if i == j { 1.0 / (i + 1) as f64 } else { 0.0 })
                    .collect()
            })
            .collect(),
        _ => vec![],
    };
    for (index, values) in auxiliary.into_iter().enumerate() {
        let source = NodeId::new();
        let id = yss_graph_document::ConstantId::new();
        let mut constant = GraphConstant {
            id,
            name: format!("auxiliary{index}"),
            data_type: ValueType::DataSeries(Box::new(ValueType::Scalar(
                yss_data_contract::SemanticType::Numeric,
            ))),
            data_value: DataValue::String(
                (serde_json::json!({"value": values}).to_string()).into(),
            ),
            tabular: None,
            description: String::new(),
            tags: vec![],
        };
        yss_graph_document::normalize_constant_value(&mut constant).unwrap();
        document.constants.insert(id, constant);
        document.nodes.insert(
            source,
            DocumentNode {
                id: source,
                node_type: "yssbi.constant.get".parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: [("constant".parse().unwrap(), id.to_string().into())].into(),
                user_label: None,
            },
        );
        let template = if method == "WLS" { "weights" } else { "sigma" };
        let input = PortAddress::instance(
            fit,
            template.parse().unwrap(),
            yss_graph_document::PortInstanceId::new(),
        );
        document.port_bindings.insert(
            input.clone(),
            yss_graph_document::DynamicPortBinding::UserCreated {
                order: yss_graph_document::OrderKey::new(index.to_string()),
            },
        );
        let id = yss_graph_document::ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(source, "value".parse().unwrap()),
                input,
                order: None,
            },
        );
    }
    (graph, document)
}
