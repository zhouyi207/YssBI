use super::fixture::{connect_port, field, node};
use super::*;
use yss_data_contract::{DataValue, ValueType};
use yss_graph_document::{DynamicPortBinding, OrderKey, PortInstanceId};

#[test]
fn panel_category_nodes_execute_defaults_and_connect_to_existing_summary() {
    yss_application::session::NodeComponents::builtins().unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/panel_category_reference.json"
    ))
    .unwrap();
    let catalog = yss_node_catalog::build_builtin_node_system().unwrap();
    for method in [
        "fe",
        "re",
        "fd",
        "between",
        "dynamic",
        "unit_root",
        "cointegration",
    ] {
        let id = format!("yssbi.statistics.econometrics.panel.{method}");
        for locale in ["en-US", "zh-CN"] {
            let localized = catalog.catalog.localize(&catalog.registry, locale);
            let item = localized
                .items
                .iter()
                .find(|item| item.node_type_id.as_ref() == id)
                .unwrap();
            assert_eq!(item.category_id.as_ref(), "statistics.panel");
            assert!(
                item.documentation
                    .as_ref()
                    .is_some_and(|doc| doc.len() > 200)
            );
        }
        let data = &fixture[if matches!(method, "unit_root" | "cointegration") {
            "nonstationary"
        } else {
            "dynamic"
        }];
        let mut document = GraphDocument::default();
        let source = node(&mut document, "yssbi.constant.get", serde_json::json!({}));
        let table = serde_json::json!({"y":data["response"],"x":data["predictors"][0],"entity":data["entity"],"time":data["time"]});
        set_constant(
            &mut document,
            source,
            ValueType::DataFrame,
            DataValue::String(table.to_string().into()),
        );
        for constant in document.constants.values_mut() {
            yss_graph_document::normalize_constant_value(constant).unwrap();
        }
        let model = node(&mut document, &id, serde_json::json!({}));
        for (column, key) in [
            ("y", if method == "unit_root" { "series" } else { "y" }),
            ("x", "x"),
            ("entity", "entity"),
            ("time", "time"),
        ] {
            if key == "x" && method == "unit_root" {
                continue;
            }
            let select = node(
                &mut document,
                "yssbi.dataframe.series.select",
                serde_json::json!({"column":column}),
            );
            connect_port(
                &mut document,
                source,
                "value",
                PortAddress::declared(select, "dataframe".parse().unwrap()),
            );
            let address = if key == "x" {
                let address =
                    PortAddress::instance(model, key.parse().unwrap(), PortInstanceId::new());
                document.port_bindings.insert(
                    address.clone(),
                    DynamicPortBinding::UserCreated {
                        order: OrderKey::new("0"),
                    },
                );
                address
            } else {
                PortAddress::declared(model, key.parse().unwrap())
            };
            connect_port(&mut document, select, "series", address);
        }
        let static_model = matches!(method, "fe" | "re" | "fd" | "between");
        let target = if static_model {
            let summary = node(
                &mut document,
                "yssbi.statistics.panel.summary",
                serde_json::json!({}),
            );
            connect_port(
                &mut document,
                model,
                "model",
                PortAddress::declared(summary, "model".parse().unwrap()),
            );
            "yssbi.statistics.panel.summary"
        } else {
            &id
        };
        let result =
            execute(&document, target).unwrap_or_else(|error| panic!("{method}: {error:?}"));
        if static_model {
            field(&result, "coefficients");
            field(&result, "estimationSample");
        } else if method == "dynamic" {
            field(&result, "coefficients");
            field(&result, "inference");
        } else {
            field(&result, "entity_tests");
            field(&result, "p_value");
        }
    }
}
