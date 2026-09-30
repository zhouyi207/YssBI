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
fn selector(document: &mut GraphDocument, source: NodeId, column: &str) -> NodeId {
    let id = node(
        document,
        "yssbi.dataframe.series.select",
        serde_json::json!({"column":column}),
    );
    connect(
        document,
        source,
        "value",
        PortAddress::declared(id, "dataframe".parse().unwrap()),
    );
    id
}
fn run_model(
    method: &str,
    case: &serde_json::Value,
    parameters: serde_json::Value,
) -> RuntimeValue {
    let mut document = GraphDocument::default();
    let source = node(&mut document, "yssbi.constant.get", serde_json::json!({}));
    let mut table = serde_json::Map::new();
    table.insert("y".into(), case["y"].clone());
    let x = case["x"].as_array().unwrap();
    for (i, column) in x.iter().enumerate() {
        table.insert(format!("x{}", i + 1), column.clone());
    }
    let n = case["y"].as_array().unwrap().len();
    let groups = case.get("groups").cloned().unwrap_or_else(|| {
        serde_json::json!(
            (0..n)
                .map(|i| format!("group{}", i % 4))
                .collect::<Vec<_>>()
        )
    });
    table.insert("g".into(), groups);
    set_constant(
        &mut document,
        source,
        ValueType::DataFrame,
        DataValue::String(serde_json::Value::Object(table).to_string().into()),
    );
    for constant in document.constants.values_mut() {
        yss_graph_document::normalize_constant_value(constant).unwrap();
    }
    let y = selector(&mut document, source, "y");
    let model = node(&mut document, method, parameters);
    connect(
        &mut document,
        y,
        "series",
        PortAddress::declared(model, "response".parse().unwrap()),
    );
    let single = matches!(
        method,
        "yssbi.statistics.regression.curve"
            | "yssbi.statistics.regression.nonlinear"
            | "yssbi.statistics.regression.deming"
            | "yssbi.statistics.transform.rcs"
    );
    for i in 0..if single { 1 } else { x.len() } {
        let id = selector(&mut document, source, &format!("x{}", i + 1));
        let address = if single {
            PortAddress::declared(model, "predictor".parse().unwrap())
        } else {
            let a =
                PortAddress::instance(model, "predictors".parse().unwrap(), PortInstanceId::new());
            document.port_bindings.insert(
                a.clone(),
                DynamicPortBinding::UserCreated {
                    order: OrderKey::new(i.to_string()),
                },
            );
            a
        };
        connect(&mut document, id, "series", address);
    }
    if method.ends_with("logit.conditional") || method.ends_with("workflow.regression.grouped") {
        let g = selector(&mut document, source, "g");
        connect(
            &mut document,
            g,
            "series",
            PortAddress::declared(model, "groups".parse().unwrap()),
        );
    }
    if method.ends_with("regression.threshold") {
        let q = selector(&mut document, source, "x1");
        connect(
            &mut document,
            q,
            "series",
            PortAddress::declared(model, "threshold_variable".parse().unwrap()),
        );
    }
    execute(&document, method).unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

#[test]
fn regression_category_nodes_execute_catalog_defaults_and_conditional_parameters() {
    yss_application::session::NodeComponents::builtins().unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../yss-sci/src/regression/models/fixtures/reference.json"
    ))
    .unwrap();
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    for (suffix, case) in [
        ("regression.robust", "huber"),
        ("regression.hierarchical", "linear"),
        ("regression.stepwise", "linear"),
        ("regression.curve", "nonlinear"),
        ("regression.nonlinear", "nonlinear"),
        ("regression.nonlinear_formula", "linear"),
        ("regression.ridge", "linear"),
        ("regression.lasso", "linear"),
        ("regression.pls", "linear"),
        ("regression.logit.multinomial", "multinomial"),
        ("regression.logit.ordinal", "ordinal"),
        ("regression.logit.firth", "logit"),
        ("regression.poisson", "poisson"),
        ("regression.negative_binomial", "negative_binomial"),
        ("regression.zero_inflated_poisson", "zip"),
        ("regression.zero_inflated_negative_binomial", "zinb"),
        ("regression.tobit", "tobit"),
        ("regression.logit.conditional", "conditional"),
        ("regression.deming", "nonlinear"),
        ("regression.quantile", "quantile"),
        ("workflow.regression.univariate_multivariable", "linear"),
        ("workflow.regression.grouped", "linear"),
        ("workflow.regression.baseline", "linear"),
        ("regression.threshold", "linear"),
        ("transform.rcs", "nonlinear"),
        ("regression.glm", "poisson"),
        ("regression.gamma", "gamma"),
        ("regression.inverse_gaussian", "inverse_gaussian"),
        ("regression.cloglog", "cloglog"),
        ("regression.beta", "beta"),
        ("regression.fractional_response", "fractional"),
    ] {
        let kind = format!("yssbi.statistics.{suffix}");
        let protocol = registry.protocol(&kind.parse().unwrap()).unwrap();
        let outputs = protocol
            .interface
            .ports
            .iter()
            .filter(|p| p.direction == yss_node_protocol::PortDirection::Output)
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].key.as_str(), "result");
        let data = if case == "linear" {
            &fixture["linear"]
        } else {
            &fixture["cases"][case]
        };
        let result = run_model(&kind, data, serde_json::json!({}));
        assert!(matches!(result, RuntimeValue::Record(_)), "{suffix}");
    }
    for (case, params) in [
        ("gaussian", serde_json::json!({"glm_family":"gaussian"})),
        (
            "gaussian_log",
            serde_json::json!({"glm_family":"gaussian","gaussian_link":"log"}),
        ),
        (
            "probit",
            serde_json::json!({"glm_family":"binomial","binomial_link":"probit"}),
        ),
    ] {
        let result = run_model(
            "yssbi.statistics.regression.glm",
            &fixture["cases"][case],
            params,
        );
        assert!(matches!(result, RuntimeValue::Record(_)));
    }
    run_model(
        "yssbi.statistics.regression.tobit",
        &fixture["cases"]["tobit_both"],
        serde_json::json!({"censoring":"both","upper":1.4}),
    );
    run_model(
        "yssbi.statistics.transform.rcs",
        &fixture["cases"]["nonlinear"],
        serde_json::json!({"knot_mode":"manual","knots":[0.4,1.4,2.6,3.8]}),
    );
}

#[test]
fn regression_models_reject_unaligned_relation_domains_and_mixed_columns() {
    use arrow::{
        array::Float64Array,
        datatypes::{DataType, Field, Schema},
        record_batch::RecordBatch,
    };
    use std::borrow::Cow;
    use yss_node_kernel::{
        KernelControl, KernelError, KernelId, KernelInvocation, KernelOutputSpec,
        KernelParameterKey, KernelRegistry,
    };
    let relations: Arc<dyn yss_relational_contract::RelationFactory> =
        yss_database_runtime::dataset_query_engine().unwrap();
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![Field::new("x", DataType::Float64, false)])),
        vec![Arc::new(Float64Array::from(vec![1., 2., 3., 4.]))],
    )
    .unwrap();
    let relation_control = yss_relational_contract::RelationControl {
        cancellation: control.cancellation.clone(),
        deadline: control.deadline,
        max_input_bytes: control.max_input_bytes,
    };
    let left = relations
        .clone()
        .materialize(batch.clone(), &relation_control)
        .unwrap();
    let right = relations
        .clone()
        .materialize(batch, &relation_control)
        .unwrap();
    let unrelated = [
        RuntimeValue::Series(left.select_series("x").unwrap()),
        RuntimeValue::Series(right.select_series("x").unwrap()),
    ];
    let mixed = [
        unrelated[0].clone(),
        RuntimeValue::List(
            [1., 2., 3., 4.]
                .into_iter()
                .map(|v| RuntimeValue::float64(v).unwrap())
                .collect(),
        ),
    ];
    let params = [
        ("constant", RuntimeValue::Scalar(TabularScalar::Bool(true))),
        (
            "standardize",
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
        ),
        ("lambda", RuntimeValue::float64(1.0).unwrap()),
    ];
    let kernels = KernelRegistry::default();
    for inputs in [&unrelated[..], &mixed[..]] {
        let inv = KernelInvocation {
            relations: &relations,
            inputs,
            input_keys: &["response", "predictors"],
            parameters: params
                .iter()
                .map(|(k, v)| {
                    (
                        KernelParameterKey::new((*k).into()).unwrap(),
                        Cow::Borrowed(v),
                    )
                })
                .collect(),
            outputs: &[KernelOutputSpec {
                data_type: ValueType::Struct("statistics.report".into()),
                fields: None,
            }],
            control: &control,
        };
        assert!(matches!(
            kernels.execute(
                &KernelId::new("yssbi.statistics.regression.ridge".into()).unwrap(),
                &inv
            ),
            Err(KernelError::UnalignedSeries)
        ));
    }
}
