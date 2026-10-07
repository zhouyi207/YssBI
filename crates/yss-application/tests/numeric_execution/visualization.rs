use super::*;
use yss_data_contract::{DataValue, SemanticType, ValueType};
use yss_graph_analysis::{GraphPlotDataKind, GraphResultCategory};

#[test]
fn visualization_nodes_resolve_execute_and_keep_plot_output_categories() {
    for (name, expected) in [
        ("histogram", GraphPlotDataKind::Histogram),
        ("kde", GraphPlotDataKind::Kde),
        ("pp_qq", GraphPlotDataKind::PpQq),
    ] {
        let mut document = GraphDocument::default();
        let source = NodeId::new();
        let target = NodeId::new();
        let kind = format!("yssbi.plot.{name}.view");
        for (id, ty) in [(source, "yssbi.constant.get"), (target, kind.as_str())] {
            document.nodes.insert(
                id,
                DocumentNode {
                    id,
                    node_type: ty.parse().unwrap(),
                    position: NodePosition { x: 0., y: 0. },
                    parameters: ParameterValues::new(),
                    user_label: None,
                },
            );
        }
        set_constant(
            &mut document,
            source,
            ValueType::DataSeries(Box::new(ValueType::Scalar(SemanticType::Numeric))),
            DataValue::String("{\"value\":[1,2,2,3,4,5,6,7]}".into()),
        );
        let constant = document
            .constants
            .get_mut(&yss_graph_document::ConstantId::from_uuid(source.as_uuid()))
            .unwrap();
        yss_graph_document::normalize_constant_value(constant).unwrap();
        let edge = ConnectionId::new();
        document.connections.insert(
            edge,
            DocumentConnection {
                id: edge,
                output: PortAddress::declared(source, "value".parse().unwrap()),
                input: PortAddress::declared(target, "values".parse().unwrap()),
                order: None,
            },
        );
        let resources = ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new());
        let analysis = analyze_document(
            &document,
            &GraphResourcePath::new("events/New Event.yssbi-event").unwrap(),
            &resources,
        );
        let snapshot = analysis.semantic_snapshot();
        assert!(
            !snapshot.has_blocking_diagnostics(),
            "{name}: {:?}",
            snapshot.diagnostics()
        );
        let node = snapshot.node(target).unwrap();
        let port = node
            .ports
            .iter()
            .find(|port| port.direction == yss_node_protocol::PortDirection::Output)
            .unwrap();
        assert_eq!(
            port.result_category,
            GraphResultCategory::PlotData(expected)
        );
        let output = execute(&document, &kind).unwrap();
        let RuntimeValue::Record(fields) = output else {
            panic!("{name} must return a plot record")
        };
        assert!(
            matches!(fields.get("data"), Some(RuntimeValue::List(values)) if !values.is_empty())
        );
    }
}
