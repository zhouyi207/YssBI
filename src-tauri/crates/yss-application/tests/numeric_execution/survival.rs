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
            position: NodePosition { x: 0.0, y: 0.0 },
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
fn port(
    document: &mut GraphDocument,
    node: NodeId,
    key: &str,
    repeat: bool,
    order: usize,
) -> PortAddress {
    if repeat {
        let address = PortAddress::instance(node, key.parse().unwrap(), PortInstanceId::new());
        document.port_bindings.insert(
            address.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(format!("{order:03}")),
            },
        );
        address
    } else {
        PortAddress::declared(node, key.parse().unwrap())
    }
}
fn field<'a>(value: &'a RuntimeValue, key: &str) -> &'a RuntimeValue {
    let RuntimeValue::Record(fields) = value else {
        panic!("record")
    };
    fields.get(key).unwrap_or_else(|| panic!("missing {key}"))
}

#[test]
fn survival_category_all_fifteen_nodes_execute_and_models_feed_evaluation_and_nomograms() {
    yss_application::session::NodeComponents::builtins().unwrap();
    let f: serde_json::Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/survival_category_reference.json"
    ))
    .unwrap();
    let catalog = yss_node_catalog::build_builtin_node_system().unwrap();
    for method in [
        "survival.kaplan_meier",
        "survival.nelson_aalen",
        "survival.logrank",
        "survival.cox",
        "survival.exponential",
        "survival.weibull",
        "survival.lognormal",
        "survival.loglogistic",
        "survival.aft",
        "survival.competing_risks",
        "survival.time_dependent_cox",
        "workflow.subgroup",
        "plot.nomogram",
        "plot.calibration",
        "plot.decision_curve",
    ] {
        let id = format!("yssbi.statistics.{method}");
        for locale in ["en-US", "zh-CN"] {
            let localized = catalog.catalog.localize(&catalog.registry, locale);
            let item = localized
                .items
                .iter()
                .find(|item| item.node_type_id.as_ref() == id)
                .unwrap();
            assert_eq!(item.category_id.as_ref(), "statistics.survival");
            assert!(
                item.documentation
                    .as_ref()
                    .is_some_and(|doc| doc.len() > 500)
            );
        }
        let td = method == "survival.time_dependent_cox";
        let plot = method.starts_with("plot.");
        let d = &f[if td { "time_dependent" } else { "data" }];
        let group = serde_json::json!(
            f["data"]["group"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| if v == 0 { "甲组" } else { "乙组" })
                .collect::<Vec<_>>()
        );
        let event = serde_json::json!(
            d["event"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() == 1.0)
                .collect::<Vec<_>>()
        );
        let mut inputs: Vec<(&str, serde_json::Value, bool)> = vec![];
        if td {
            inputs.push(("start", d["start"].clone(), false));
        }
        inputs.push((
            if td { "stop" } else { "time" },
            d[if td { "stop" } else { "time" }].clone(),
            false,
        ));
        if method == "survival.competing_risks" {
            inputs.push((
                "status",
                serde_json::json!(
                    d["event"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .enumerate()
                        .map(|(i, e)| if e == 0 { 0 } else { 1 + i % 2 })
                        .collect::<Vec<_>>()
                ),
                false,
            ));
        } else {
            inputs.push(("event", event, false));
        }
        match method {
            "survival.kaplan_meier" | "survival.nelson_aalen" => {
                inputs.push(("groups", group, true))
            }
            "survival.logrank" => inputs.push(("groups", group, false)),
            "workflow.subgroup" => inputs.extend([
                ("treatment", d["treatment"].clone(), false),
                ("groups", group, false),
            ]),
            "survival.time_dependent_cox" => {
                inputs.push(("subjects", d["subjects"].clone(), false))
            }
            _ => {}
        }
        if !matches!(
            method,
            "survival.kaplan_meier"
                | "survival.nelson_aalen"
                | "survival.logrank"
                | "survival.competing_risks"
        ) {
            for x in d["predictors"].as_array().unwrap() {
                inputs.push(("predictors", x.clone(), true));
            }
        }
        let mut document = GraphDocument::default();
        let source = node(&mut document, "yssbi.constant.get", serde_json::json!({}));
        let table = serde_json::Value::Object(
            inputs
                .iter()
                .enumerate()
                .map(|(i, (_, v, _))| (format!("column{i}"), v.clone()))
                .collect(),
        );
        set_constant(
            &mut document,
            source,
            ValueType::DataFrame,
            DataValue::String(table.to_string().into()),
        );
        for c in document.constants.values_mut() {
            yss_graph_document::normalize_constant_value(c).unwrap();
        }
        let upstream = if plot {
            "yssbi.statistics.survival.cox"
        } else {
            &id
        };
        let model = node(&mut document, upstream, serde_json::json!({}));
        for (i, (key, _, repeat)) in inputs.iter().enumerate() {
            let select = node(
                &mut document,
                "yssbi.dataframe.series.select",
                serde_json::json!({"column":format!("column{i}")}),
            );
            connect(
                &mut document,
                source,
                "value",
                PortAddress::declared(select, "dataframe".parse().unwrap()),
            );
            let address = port(&mut document, model, key, *repeat, i);
            connect(&mut document, select, "series", address);
        }
        if plot {
            let target = node(&mut document, &id, serde_json::json!({}));
            if method == "plot.nomogram" {
                connect(
                    &mut document,
                    model,
                    "result",
                    PortAddress::declared(target, "model".parse().unwrap()),
                );
            } else {
                for (column, key) in [
                    ("time", "time"),
                    ("event", "event"),
                    ("risk", "predicted_risk"),
                ] {
                    let select = node(
                        &mut document,
                        "yssbi.dataframe.series.select",
                        serde_json::json!({"column":column}),
                    );
                    connect(
                        &mut document,
                        model,
                        "predictions",
                        PortAddress::declared(select, "dataframe".parse().unwrap()),
                    );
                    connect(
                        &mut document,
                        select,
                        "series",
                        PortAddress::declared(target, key.parse().unwrap()),
                    );
                }
            }
        }
        let result = execute(&document, &id).unwrap_or_else(|e| panic!("{method}: {e:?}"));
        let key = match method {
            "survival.kaplan_meier" | "survival.nelson_aalen" => "curves",
            "survival.logrank" => "test",
            "survival.competing_risks" => "points",
            "workflow.subgroup" => "equality_test",
            "plot.nomogram" => "axes",
            "plot.calibration" => "bins",
            "plot.decision_curve" => "estimates",
            _ => "coefficients",
        };
        assert!(!matches!(
            field(&result, key),
            RuntimeValue::Scalar(yss_data_contract::TabularScalar::Null)
        ));
    }
}
