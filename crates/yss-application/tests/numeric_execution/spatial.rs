use super::*;
use yss_data_contract::{DataValue, ValueType};
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
fn connect(document: &mut GraphDocument, from: NodeId, key: &str, to: PortAddress) {
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(from, key.parse().unwrap()),
            input: to,
            order: None,
        },
    );
}
fn table(document: &mut GraphDocument, columns: serde_json::Value) -> NodeId {
    let source = node(document, "yssbi.constant.get", serde_json::json!({}));
    set_constant(
        document,
        source,
        ValueType::DataFrame,
        DataValue::String(columns.to_string().into()),
    );
    for c in document.constants.values_mut() {
        yss_graph_document::normalize_constant_value(c).unwrap();
    }
    source
}
fn select(
    document: &mut GraphDocument,
    source: NodeId,
    column: &str,
    target: NodeId,
    key: &str,
    order: Option<usize>,
) {
    let selector = node(
        document,
        "yssbi.dataframe.series.select",
        serde_json::json!({"column":column}),
    );
    connect(
        document,
        source,
        "value",
        PortAddress::declared(selector, "dataframe".parse().unwrap()),
    );
    let input = if let Some(order) = order {
        let address = PortAddress::instance(target, key.parse().unwrap(), PortInstanceId::new());
        document.port_bindings.insert(
            address.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(format!("{order:03}")),
            },
        );
        address
    } else {
        PortAddress::declared(target, key.parse().unwrap())
    };
    connect(document, selector, "series", input);
}
fn field<'a>(value: &'a RuntimeValue, key: &str) -> &'a RuntimeValue {
    let RuntimeValue::Record(fields) = value else {
        panic!("record")
    };
    fields.get(key).unwrap_or_else(|| panic!("missing {key}"))
}
fn scalar(v: &RuntimeValue) -> f64 {
    match v {
        RuntimeValue::Scalar(TabularScalar::Float64(v)) => v.as_f64(),
        RuntimeValue::Scalar(TabularScalar::Integer(v)) => *v as f64,
        _ => panic!("number: {v:?}"),
    }
}

#[test]
fn spatial_category_all_ten_nodes_execute_with_typed_weights_and_aligned_tables() {
    yss_application::session::NodeComponents::builtins().unwrap();
    let f: serde_json::Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/spatial_category_reference.json"
    ))
    .unwrap();
    let d = &f["data"];
    let n = d["x"].as_array().unwrap().len();
    let catalog = yss_node_catalog::build_builtin_node_system().unwrap();
    for method in [
        "weights", "moran", "ols", "slm", "sem", "sac", "sdm", "sdem", "slx", "panel",
    ] {
        let id = format!("yssbi.statistics.spatial.{method}");
        for locale in ["zh-CN", "en-US"] {
            let localized = catalog.catalog.localize(&catalog.registry, locale);
            let item = localized
                .items
                .iter()
                .find(|item| item.node_type_id.as_ref() == id)
                .unwrap();
            assert_eq!(item.category_id.as_ref(), "statistics.spatial");
            assert!(item.documentation.as_ref().is_some_and(|d| d.len() > 500));
        }
        let mut document = GraphDocument::default();
        let coordinate_table = table(
            &mut document,
            serde_json::json!({
            "district":(0..n).map(|i|format!("地区-{i}")).collect::<Vec<_>>(),
            "east":d["x"],"north":d["y_coordinate"]}),
        );
        let weights = node(
            &mut document,
            "yssbi.statistics.spatial.weights",
            serde_json::json!({}),
        );
        for (column, key) in [("district", "units"), ("east", "x"), ("north", "y")] {
            select(&mut document, coordinate_table, column, weights, key, None);
        }
        if method != "weights" {
            let data = if method == "panel" { &f["panel"] } else { d };
            let rows = data["response"].as_array().unwrap().len();
            let reverse = |v: &serde_json::Value| {
                serde_json::json!(v.as_array().unwrap().iter().rev().collect::<Vec<_>>())
            };
            let mut columns = serde_json::json!({
                "district":(0..rows).rev().map(|i|format!("地区-{}",i%n)).collect::<Vec<_>>(),
                "y":reverse(&data["response"]),
                "income":reverse(&data["predictors"][0]),"density":reverse(&data["predictors"][1])});
            if method == "panel" {
                columns["period"] = serde_json::json!(
                    (0..rows)
                        .rev()
                        .map(|i| format!("期-{}", i / n))
                        .collect::<Vec<_>>()
                );
            }
            let source = table(&mut document, columns);
            let target = node(&mut document, &id, serde_json::json!({}));
            connect(
                &mut document,
                weights,
                "result",
                PortAddress::declared(target, "weights".parse().unwrap()),
            );
            for (column, key) in [("district", "units"), ("y", "y")] {
                select(&mut document, source, column, target, key, None);
            }
            if method == "panel" {
                select(&mut document, source, "period", target, "periods", None);
            }
            if method != "moran" {
                select(&mut document, source, "income", target, "x", Some(0));
                select(&mut document, source, "density", target, "x", Some(1));
            }
        }
        let result = execute(&document, &id).unwrap_or_else(|e| panic!("{method}: {e:?}"));
        if method == "weights" {
            let RuntimeValue::List(matrix) = field(&result, "matrix") else {
                panic!("matrix")
            };
            assert_eq!(matrix.len(), n);
        } else if method == "moran" {
            assert!(
                (scalar(field(&result, "statistic")) - f["moran"]["statistic"].as_f64().unwrap())
                    .abs()
                    < 1e-12
            );
        } else {
            let expected = if method == "panel" {
                &f["panel"]["models"]["slm"]
            } else {
                &f["models"][method]
            };
            assert!(
                (scalar(field(&result, "log_likelihood"))
                    - expected["log_likelihood"].as_f64().unwrap())
                .abs()
                    < 1e-5,
                "{method}"
            );
            let RuntimeValue::List(coefficients) = field(&result, "coefficients") else {
                panic!("coefficients")
            };
            let coefficient = &coefficients[usize::from(method != "panel")];
            assert_eq!(
                field(coefficient, "term"),
                &RuntimeValue::Scalar(TabularScalar::String("income".into()))
            );
            let RuntimeValue::List(units) = field(&result, "unit_labels") else {
                panic!("units")
            };
            assert_eq!(units.len(), n);
        }
    }
}
