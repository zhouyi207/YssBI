use super::*;
use yss_data_contract::{DataValue, ValueType};
use yss_graph_analysis::{GraphPlotDataKind, GraphResultCategory, GraphStatisticalReportKind};
use yss_graph_document::{DynamicPortBinding, OrderKey, PortInstanceId};

fn node(document: &mut GraphDocument, kind: &str, parameters: serde_json::Value) -> NodeId {
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
fn connect(document: &mut GraphDocument, from: NodeId, key: &str, input: PortAddress) {
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(from, key.parse().unwrap()),
            input,
            order: None,
        },
    );
}
#[test]
fn time_series_category_defaults_execute_relational_inputs_and_publish_reports_or_plots() {
    yss_application::session::NodeComponents::builtins().unwrap();
    let f: serde_json::Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/time_series_reference.json"
    ))
    .unwrap();
    let catalog = yss_node_catalog::build_builtin_node_system().unwrap();
    for method in [
        "arima",
        "sarima",
        "ecm",
        "arch",
        "garch",
        "egarch",
        "gjr_garch",
        "grey_prediction",
        "exponential_smoothing",
        "ets",
        "holt_winters",
        "markov_prediction",
        "phillips_perron",
        "kpss",
        "correlogram",
        "time_series",
    ] {
        let plot = matches!(method, "correlogram" | "time_series");
        let kind = format!(
            "yssbi.statistics.{}.{method}",
            if plot { "plot" } else { "timeseries" }
        );
        for locale in ["en-US", "zh-CN"] {
            let localized = catalog.catalog.localize(&catalog.registry, locale);
            let item = localized
                .items
                .iter()
                .find(|i| i.node_type_id.as_ref() == kind)
                .unwrap();
            assert_eq!(item.category_id.as_ref(), "statistics.timeseries");
            assert!(
                item.documentation
                    .as_ref()
                    .is_some_and(|doc| doc.len() > 200 && !doc.contains("范围待确认")),
                "{kind}: {locale}"
            );
        }
        let y = if method == "ecm" {
            f["ecm"]["y"].clone()
        } else if matches!(method, "arch" | "garch" | "egarch" | "gjr_garch") {
            f["volatility"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["method"] == method)
                .unwrap()["y"]
                .clone()
        } else if method == "grey_prediction" {
            f["grey"]["y"].clone()
        } else if method == "markov_prediction" {
            serde_json::json!(["晴", "雨", "晴", "晴", "雨", "雨", "晴"])
        } else {
            f["series"].clone()
        };
        let n = y.as_array().unwrap().len();
        let table = if method == "ecm" {
            serde_json::json!({"y":y,"x":f["ecm"]["x"]})
        } else {
            serde_json::json!({"y":y,"time":(1..=n).collect::<Vec<_>>()})
        };
        let mut document = GraphDocument::default();
        let source = node(&mut document, "yssbi.constant.get", serde_json::json!({}));
        set_constant(
            &mut document,
            source,
            ValueType::DataFrame,
            DataValue::String(table.to_string().into()),
        );
        for constant in document.constants.values_mut() {
            yss_graph_document::normalize_constant_value(constant).unwrap();
        }
        let model = node(&mut document, &kind, serde_json::json!({}));
        let mut inputs = vec![("series", "y")];
        if method == "ecm" {
            inputs.push(("x", "x"));
        }
        if method == "time_series" {
            inputs.push(("time", "time"));
        }
        for (key, column) in inputs {
            let select = node(
                &mut document,
                "yssbi.dataframe.series.select",
                serde_json::json!({"column":column}),
            );
            connect(
                &mut document,
                source,
                "value",
                PortAddress::declared(select, "dataframe".parse().unwrap()),
            );
            let address = if key == "series" {
                PortAddress::declared(model, key.parse().unwrap())
            } else {
                let address =
                    PortAddress::instance(model, key.parse().unwrap(), PortInstanceId::new());
                document.port_bindings.insert(
                    address.clone(),
                    DynamicPortBinding::UserCreated {
                        order: OrderKey::new("0"),
                    },
                );
                address
            };
            connect(&mut document, select, "series", address);
        }
        let resources = ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new());
        let analysis = analyze_document(
            &document,
            &GraphResourcePath::new("events/New Event.yssbi-event").unwrap(),
            &resources,
        );
        let snapshot = analysis.semantic_snapshot();
        assert!(
            !snapshot.has_blocking_diagnostics(),
            "{method}: {:?}",
            snapshot.diagnostics()
        );
        let expected = match method {
            "time_series" => GraphResultCategory::PlotData(GraphPlotDataKind::Line),
            "correlogram" => GraphResultCategory::PlotData(GraphPlotDataKind::Correlogram),
            _ => GraphResultCategory::StatisticalReport(GraphStatisticalReportKind::Structured),
        };
        assert_eq!(
            snapshot
                .node(model)
                .unwrap()
                .ports
                .iter()
                .find(|p| p.direction == yss_node_protocol::PortDirection::Output)
                .unwrap()
                .result_category,
            expected
        );
        let output = execute(&document, &kind).unwrap_or_else(|e| panic!("{method}: {e:?}"));
        let RuntimeValue::Record(fields) = output else {
            panic!("{method}: expected record")
        };
        let field = match method {
            "time_series" => "data",
            "correlogram" => "acf",
            "ecm" => "short_run",
            "markov_prediction" => "forecast_labels",
            "phillips_perron" | "kpss" => "p_value",
            "arch" | "garch" | "egarch" | "gjr_garch" => "forecast_variances",
            _ => "forecasts",
        };
        assert!(fields.contains_key(field), "{method}: missing {field}");
    }
}
