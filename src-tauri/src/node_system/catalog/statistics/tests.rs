use super::{
    LEGACY_NODE_IDS, NODES, build_provider_fragment, families::Family, prediction_model_type,
};
use crate::node_system::catalog::builtin::BuiltinAssemblyError;
use crate::node_system::protocol::{
    LiteralPolicy, NodeInterfaceProtocol, ParameterKey, PortDirection, PortInstances, TypeExpr,
    TypeId, data_series_type, numeric_data_series_type, validate_parameter_values,
};
use std::collections::BTreeSet;

#[test]
fn every_legacy_statistics_node_has_one_stable_id() {
    assert_eq!(LEGACY_NODE_IDS.len(), 42);
    assert_eq!(NODES.len(), LEGACY_NODE_IDS.len());

    let legacy = LEGACY_NODE_IDS
        .iter()
        .map(|(name, _)| *name)
        .collect::<BTreeSet<_>>();
    let ids = LEGACY_NODE_IDS
        .iter()
        .map(|(_, id)| *id)
        .collect::<BTreeSet<_>>();

    assert_eq!(legacy.len(), LEGACY_NODE_IDS.len());
    assert_eq!(ids.len(), LEGACY_NODE_IDS.len());
    assert!(ids.iter().all(|id| id.starts_with("yssbi.statistics.")));
    for spec in NODES {
        assert_eq!(
            LEGACY_NODE_IDS
                .iter()
                .find(|(legacy_name, _)| *legacy_name == spec.legacy_name)
                .map(|(_, id)| *id),
            Some(spec.id),
        );
    }
}

#[test]
fn statistics_fragment_contains_every_migrated_protocol() {
    let fragment = build_provider_fragment().expect("statistics built-in fixture must assemble");
    assert_eq!(fragment.nodes.len(), LEGACY_NODE_IDS.len());
}

#[test]
fn ols_summary_accepts_numeric_data_series_and_rejects_string_series() {
    let fragment = build_provider_fragment().expect("statistics built-in fixture must assemble");
    let summary = fragment
        .nodes
        .iter()
        .find(|node| node.protocol().type_id.as_str() == "yssbi.statistics.ols.summary")
        .expect("OLS Summary protocol");
    let port_type = |key: &str| {
        summary
            .protocol()
            .interface
            .ports
            .iter()
            .find(|port| port.key.as_str() == key)
            .unwrap_or_else(|| panic!("OLS Summary must expose {key}"))
            .value_type
            .clone()
    };
    let series = data_series_type;
    let int = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let string = TypeExpr::Concrete(TypeId::new("core.string").unwrap());
    let numeric_union = numeric_data_series_type();

    for key in ["response", "predictors"] {
        let target = port_type(key);
        assert!(crate::node_system::compiler::type_exprs_assignable(
            &series(int.clone()),
            &target,
            &[],
            &[],
        ));
        assert!(crate::node_system::compiler::type_exprs_assignable(
            &series(float.clone()),
            &target,
            &[],
            &[],
        ));
        assert!(crate::node_system::compiler::type_exprs_assignable(
            &numeric_union,
            &target,
            &[],
            &[],
        ));
        assert!(!crate::node_system::compiler::type_exprs_assignable(
            &series(string.clone()),
            &target,
            &[],
            &[],
        ));
    }
}

#[test]
fn statistics_protocols_use_outer_numeric_unions_and_float_series_outputs() {
    let fragment = build_provider_fragment().expect("statistics built-in fixture must assemble");
    let numeric = numeric_data_series_type();
    let float_series = data_series_type(TypeExpr::Concrete(TypeId::new("core.float64").unwrap()));

    for node in &fragment.nodes {
        for port in &node.protocol().interface.ports {
            if port.kind != crate::node_system::protocol::PortKind::Data {
                continue;
            }
            if matches!(
                port.key.as_str(),
                "response"
                    | "predictors"
                    | "weights"
                    | "variables"
                    | "endogenous"
                    | "instruments"
                    | "entity"
                    | "time"
                    | "treatment"
                    | "series"
            ) {
                assert_eq!(
                    port.value_type,
                    numeric,
                    "{}:{}",
                    node.protocol().type_id,
                    port.key
                );
            }
            if port.direction == PortDirection::Output
                && matches!(port.key.as_str(), "fitted" | "residuals" | "prediction")
            {
                assert_eq!(
                    port.value_type,
                    float_series,
                    "{}:{} must be Float64 DataSeries",
                    node.protocol().type_id,
                    port.key
                );
            }
        }
    }
}

#[test]
fn unsupported_prediction_family_cannot_silently_use_ols() {
    assert_eq!(
        prediction_model_type(Family::Panel),
        Err(
            BuiltinAssemblyError::UnsupportedStatisticsPredictionFamily {
                family: "Panel".into(),
            }
        )
    );
}

#[test]
fn statistics_function_abi_preserves_family_nominals_and_rejects_cross_family_binding() {
    let fragment = build_provider_fragment().expect("statistics built-in fixture must assemble");
    let port_type = |node_id: &str, key: &str| {
        fragment
            .nodes
            .iter()
            .find(|node| node.protocol().type_id.as_str() == node_id)
            .unwrap_or_else(|| panic!("missing {node_id}"))
            .protocol()
            .interface
            .ports
            .iter()
            .find(|port| port.key.as_str() == key)
            .unwrap_or_else(|| panic!("missing {node_id}:{key}"))
            .value_type
            .clone()
    };
    let nominal = |id| TypeExpr::Concrete(TypeId::new(id).unwrap());

    let ols_model = port_type("yssbi.statistics.ols.fit", "model");
    let logit_model = port_type("yssbi.statistics.logit.fit", "model");
    let logit_predict_model = port_type("yssbi.statistics.logit.predict", "model");
    let iv_2sls_result = port_type("yssbi.statistics.iv.2sls.summary", "result");

    assert_eq!(ols_model, nominal("statistics.model.ols"));
    assert_eq!(logit_model, nominal("statistics.model.logit"));
    assert_eq!(logit_predict_model, nominal("statistics.model.logit"));
    assert_eq!(iv_2sls_result, nominal("statistics.model.iv_2sls"));
    assert!(!crate::node_system::compiler::type_exprs_assignable(
        &ols_model,
        &logit_predict_model,
        &[],
        &[],
    ));
    assert!(!crate::node_system::compiler::type_exprs_assignable(
        &iv_2sls_result,
        &logit_predict_model,
        &[],
        &[],
    ));
}

#[test]
fn iv_summary_requires_exactly_one_endogenous_and_one_instrument() {
    let fragment = build_provider_fragment().expect("statistics built-in fixture must assemble");
    for node_id in [
        "yssbi.statistics.iv.2sls.summary",
        "yssbi.statistics.iv.liml.summary",
    ] {
        let node = fragment
            .nodes
            .iter()
            .find(|node| node.protocol().type_id.as_str() == node_id)
            .unwrap_or_else(|| panic!("missing {node_id}"));
        for key in ["endogenous", "instruments"] {
            let port = node
                .protocol()
                .interface
                .ports
                .iter()
                .find(|port| port.key.as_str() == key)
                .unwrap_or_else(|| panic!("missing {node_id}:{key}"));
            assert_eq!(
                port.instances,
                PortInstances::UserCreated {
                    min: 1,
                    max: Some(1)
                }
            );
        }
    }
}

#[test]
fn default_statistics_node_allows_both_setting_overrides_to_inherit() {
    let fragment = build_provider_fragment().expect("statistics built-in fixture must assemble");
    let fit = fragment
        .nodes
        .iter()
        .find(|node| node.protocol().type_id.as_str() == "yssbi.statistics.ols.fit")
        .expect("OLS fit protocol");
    let values = std::collections::BTreeMap::<ParameterKey, serde_json::Value>::new();

    fn no_nominal_validation(_: &TypeId, _: &serde_json::Value) -> Option<Result<(), String>> {
        None
    }
    let issues = validate_parameter_values(fit.protocol(), &values, &no_nominal_validation);

    assert!(
        issues.is_empty(),
        "absent overrides must inherit: {issues:?}"
    );
}

#[test]
fn every_graph_crossing_statistics_model_or_result_is_family_specific() {
    let fragment = build_provider_fragment().expect("statistics built-in fixture must assemble");
    let output_type = |node_id: &str, key: &str| {
        fragment
            .nodes
            .iter()
            .find(|node| node.protocol().type_id.as_str() == node_id)
            .unwrap_or_else(|| panic!("missing {node_id}"))
            .protocol()
            .interface
            .ports
            .iter()
            .find(|port| port.key.as_str() == key)
            .unwrap_or_else(|| panic!("missing {node_id}:{key}"))
            .value_type
            .clone()
    };

    let cases = [
        ("yssbi.statistics.ols.fit", "model", "statistics.model.ols"),
        (
            "yssbi.statistics.iv.2sls.summary",
            "result",
            "statistics.model.iv_2sls",
        ),
        (
            "yssbi.statistics.iv.liml.summary",
            "result",
            "statistics.model.iv_liml",
        ),
        (
            "yssbi.statistics.panel.summary",
            "result",
            "statistics.model.panel",
        ),
        (
            "yssbi.statistics.panel.did.twfe",
            "result",
            "statistics.model.panel_did",
        ),
        (
            "yssbi.statistics.var.summary",
            "result",
            "statistics.model.var",
        ),
        (
            "yssbi.statistics.adf.test",
            "result",
            "statistics.model.adf",
        ),
        (
            "yssbi.statistics.vec.rank_test",
            "result",
            "statistics.model.vec_rank",
        ),
    ];
    for (node_id, key, expected) in cases {
        assert_eq!(
            output_type(node_id, key),
            TypeExpr::Concrete(TypeId::new(expected).unwrap()),
            "{node_id}:{key}"
        );
    }
}

#[test]
fn statistics_protocols_have_unique_ports_and_valid_bindings() {
    for node in build_provider_fragment()
        .expect("statistics built-in fixture must assemble")
        .nodes
    {
        let protocol = &node.protocol();
        let keys = protocol
            .interface
            .ports
            .iter()
            .map(|port| &port.key)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            keys.len(),
            protocol.interface.ports.len(),
            "{}",
            protocol.type_id
        );

        NodeInterfaceProtocol::new(
            protocol.interface.ports.to_vec(),
            protocol.interface.type_parameters.to_vec(),
            protocol.interface.type_constraints.to_vec(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", protocol.type_id));

        for port in &protocol.interface.ports {
            if let Some(default) = port
                .input_binding
                .as_ref()
                .and_then(|binding| binding.default_value.as_ref())
            {
                assert_eq!(
                    port.input_binding.as_ref().unwrap().literal_policy,
                    LiteralPolicy::Allowed,
                    "{}:{}",
                    protocol.type_id,
                    port.key
                );
                assert_eq!(
                    default.value_type, port.value_type,
                    "{}:{}",
                    protocol.type_id, port.key
                );
            }
        }
    }
}
