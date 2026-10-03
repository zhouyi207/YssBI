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
fn connect(document: &mut GraphDocument, from: NodeId, output: &str, to: PortAddress) {
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(from, output.parse().unwrap()),
            input: to,
            order: None,
        },
    );
}
fn repeated(
    inputs: &mut Vec<(&'static str, serde_json::Value, bool)>,
    key: &'static str,
    columns: &serde_json::Value,
) {
    for column in columns.as_array().unwrap() {
        inputs.push((key, column.clone(), true));
    }
}
fn field<'a>(value: &'a RuntimeValue, key: &str) -> &'a RuntimeValue {
    let RuntimeValue::Record(record) = value else {
        panic!("record")
    };
    record.get(key).unwrap_or_else(|| panic!("missing {key}"))
}

#[test]
fn causal_category_nodes_execute_catalog_defaults_and_typed_effect_connections() {
    yss_application::session::NodeComponents::builtins().unwrap();
    let f: serde_json::Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/causal_category_reference.json"
    ))
    .unwrap();
    let catalog = yss_node_catalog::build_builtin_node_system().unwrap();
    for method in [
        "econometrics.gmm",
        "causal.rdd",
        "causal.psm",
        "econometrics.heckman_two_step",
        "test.heterogeneity",
        "econometrics.sfa",
        "econometrics.sur",
        "causal.ipw",
        "causal.regression_adjustment",
        "causal.aipw",
        "causal.ate",
        "causal.att",
        "causal.synthetic_control",
        "panel.did.twfe",
        "iv.2sls.fit",
        "iv.liml.fit",
    ] {
        let id = format!("yssbi.statistics.{method}");
        for locale in ["en-US", "zh-CN"] {
            let localized = catalog.catalog.localize(&catalog.registry, locale);
            let item = localized
                .items
                .iter()
                .find(|item| item.node_type_id.as_ref() == id)
                .unwrap();
            assert_eq!(item.category_id.as_ref(), "statistics.causal");
            assert!(
                item.documentation
                    .as_ref()
                    .is_some_and(|doc| doc.len() > 500)
            );
        }
        let mut inputs = vec![];
        let projection = matches!(method, "causal.ate" | "causal.att");
        match method {
            "iv.2sls.fit" | "iv.liml.fit" => {
                let signal = |row: usize, bit: usize| {
                    if row & (1 << bit) == 0 { -1.0 } else { 1.0 }
                };
                let x = (0..64).map(|i| signal(i, 0)).collect::<Vec<_>>();
                let z1 = (0..64).map(|i| signal(i, 1)).collect::<Vec<_>>();
                let z2 = (0..64).map(|i| signal(i, 2)).collect::<Vec<_>>();
                let endogenous = (0..64)
                    .map(|i| 2. * z1[i] + 0.5 * z2[i] + 0.3 * x[i] + signal(i, 3))
                    .collect::<Vec<_>>();
                let response = (0..64)
                    .map(|i| {
                        1. + 0.7 * x[i]
                            + 2. * endogenous[i]
                            + 0.8 * signal(i, 3)
                            + 0.5 * signal(i, 4)
                            + signal(i, 5)
                    })
                    .collect::<Vec<_>>();
                inputs.extend([
                    ("response", serde_json::json!(response), false),
                    ("predictors", serde_json::json!(x), true),
                    ("endogenous", serde_json::json!(endogenous), true),
                    ("instruments", serde_json::json!(z1), true),
                    ("instruments", serde_json::json!(z2), true),
                ]);
            }
            "panel.did.twfe" => {
                let entity = (0..48).map(|i| f64::from(i / 6)).collect::<Vec<_>>();
                let time = (0..48).map(|i| f64::from(i % 6)).collect::<Vec<_>>();
                let treatment = (0..48)
                    .map(|i| f64::from(entity[i] < 3. && time[i] >= 3.))
                    .collect::<Vec<_>>();
                let response = (0..48)
                    .map(|i| entity[i] * 0.4 + time[i] * 0.3 + 1.75 * treatment[i])
                    .collect::<Vec<_>>();
                inputs.extend([
                    ("response", serde_json::json!(response), false),
                    ("entity", serde_json::json!(entity), false),
                    ("time", serde_json::json!(time), false),
                    ("treatment", serde_json::json!(treatment), false),
                ]);
            }
            "econometrics.sur" => {
                repeated(&mut inputs, "responses", &f["sur"]["responses"]);
                repeated(&mut inputs, "predictors", &f["sur"]["predictors"]);
            }
            "econometrics.gmm" => {
                let d = &f["gmm"];
                inputs.push(("response", d["response"].clone(), false));
                repeated(&mut inputs, "predictors", &d["predictors"]);
                repeated(&mut inputs, "instruments", &d["instruments"]);
            }
            "causal.rdd" => {
                let d = &f["rdd"];
                inputs.extend([
                    ("response", d["response"].clone(), false),
                    ("running", d["running"].clone(), false),
                ]);
            }
            "econometrics.sfa" => {
                let d = &f["frontier"];
                inputs.push(("response", d["response"].clone(), false));
                repeated(&mut inputs, "predictors", &d["predictors"]);
            }
            "econometrics.heckman_two_step" => {
                let d = &f["heckman"];
                inputs.extend([
                    ("response", d["response"].clone(), false),
                    ("selected", d["selected"].clone(), false),
                ]);
                repeated(&mut inputs, "predictors", &d["predictors"]);
                repeated(
                    &mut inputs,
                    "selection_predictors",
                    &d["selection_predictors"],
                );
            }
            "causal.synthetic_control" => {
                let d = &f["synthetic"];
                inputs.push(("response", d["response"].clone(), false));
                repeated(&mut inputs, "donors", &d["donors"]);
            }
            _ => {
                let d = &f["treatment"];
                inputs.extend([
                    ("response", d["response"].clone(), false),
                    ("treatment", d["treatment"].clone(), false),
                ]);
                if method == "test.heterogeneity" {
                    inputs.push((
                        "groups",
                        serde_json::json!(
                            f["heterogeneity"]["groups"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|v| if v == 0 { "东北" } else { "西南" })
                                .collect::<Vec<_>>()
                        ),
                        false,
                    ));
                }
                repeated(&mut inputs, "predictors", &d["predictors"]);
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
        for constant in document.constants.values_mut() {
            yss_graph_document::normalize_constant_value(constant).unwrap();
        }
        let model = node(
            &mut document,
            if projection {
                "yssbi.statistics.causal.aipw"
            } else {
                &id
            },
            serde_json::json!({}),
        );
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
            let address = if *repeat {
                let address =
                    PortAddress::instance(model, (*key).parse().unwrap(), PortInstanceId::new());
                document.port_bindings.insert(
                    address.clone(),
                    DynamicPortBinding::UserCreated {
                        order: OrderKey::new(format!("{i:03}")),
                    },
                );
                address
            } else {
                PortAddress::declared(model, (*key).parse().unwrap())
            };
            connect(&mut document, select, "series", address);
        }
        if projection {
            let target = node(&mut document, &id, serde_json::json!({}));
            connect(
                &mut document,
                model,
                "result",
                PortAddress::declared(target, "effects".parse().unwrap()),
            );
        }
        if matches!(method, "iv.2sls.fit" | "iv.liml.fit") {
            let summary_id = id.replace(".fit", ".summary");
            let summary = node(
                &mut document,
                &summary_id,
                serde_json::json!({"first_stage":true}),
            );
            connect(
                &mut document,
                model,
                "model",
                PortAddress::declared(summary, "model".parse().unwrap()),
            );
            let report =
                execute(&document, &summary_id).unwrap_or_else(|e| panic!("{method}: {e:?}"));
            assert_eq!(
                field(&report, "responseName"),
                &RuntimeValue::Scalar(TabularScalar::String("column0".into()))
            );
            assert_eq!(
                field(field(&report, "coefficients"), "labels"),
                &RuntimeValue::List(
                    ["_cons", "column1", "column2"]
                        .into_iter()
                        .map(|name| RuntimeValue::Scalar(TabularScalar::String(name.into())))
                        .collect(),
                )
            );
            let RuntimeValue::List(equations) = field(field(&report, "firstStage"), "equations")
            else {
                panic!("first-stage equations")
            };
            assert_eq!(equations.len(), 1);
            assert_eq!(
                field(&equations[0], "endog_name"),
                &RuntimeValue::Scalar(TabularScalar::String("column2".into()))
            );
            assert_eq!(
                field(&equations[0], "var_names"),
                &RuntimeValue::List(
                    ["_cons", "column1", "column3", "column4"]
                        .into_iter()
                        .map(|name| RuntimeValue::Scalar(TabularScalar::String(name.into())))
                        .collect(),
                )
            );
            field(&report, "report_display");
            continue;
        }
        let r = execute(&document, &id).unwrap_or_else(|e| panic!("{method}: {e:?}"));
        if method == "panel.did.twfe" {
            let model = field(&r, "model");
            assert_eq!(
                field(model, "family"),
                &RuntimeValue::Scalar(TabularScalar::String("panel_did_twfe".into()))
            );
            let RuntimeValue::List(names) = field(model, "parameterNames") else {
                panic!("parameter names")
            };
            let treatment_index = names
                .iter()
                .position(|name| name == &RuntimeValue::Scalar(TabularScalar::String("ATT".into())))
                .unwrap();
            let RuntimeValue::List(coefficients) = field(model, "coefficients") else {
                panic!("coefficients")
            };
            assert!(matches!(
                &coefficients[treatment_index],
                RuntimeValue::Scalar(TabularScalar::Float64(value))
                    if (value.as_f64() - 1.75).abs() < 1e-10
            ));
            field(&r, "summary");
            field(&r, "report_display");
            continue;
        }
        let key = match method {
            "causal.ate" | "causal.att" => "effect",
            "causal.psm" | "causal.ipw" | "causal.regression_adjustment" | "causal.aipw" => "ate",
            "econometrics.heckman_two_step" => "outcome_coefficients",
            "econometrics.sur" => "equations",
            "causal.synthetic_control" => "donor_weights",
            _ => "coefficients",
        };
        field(&r, key);
        if method == "test.heterogeneity" {
            let RuntimeValue::List(groups) = field(&r, "groups") else {
                panic!("groups")
            };
            assert_eq!(groups.len(), 2);
        }
    }
}
