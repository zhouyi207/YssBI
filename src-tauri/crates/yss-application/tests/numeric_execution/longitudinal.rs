use super::regression_models::{connect, node, selector};
use super::*;
use yss_data_contract::{DataValue, ValueType};
use yss_graph_document::{DynamicPortBinding, OrderKey, PortInstanceId};

#[test]
fn longitudinal_category_executes_all_ten_nodes_with_aligned_relations_and_catalog_defaults() {
    yss_application::session::NodeComponents::builtins().unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../yss-sci/src/longitudinal/fixtures/reference.json"
    ))
    .unwrap();
    for (suffix, name) in [
        ("longitudinal.gee", "gee_gaussian"),
        ("mixed.hlm", "nested"),
        ("mixed.lmm", "lmm_reml"),
        ("mixed.glmm", "glmm_binomial"),
        ("mixed.random_intercept", "lmm_reml"),
        ("mixed.random_slope", "random_slope"),
        ("mixed.crossed_effects", "crossed"),
        ("mixed.logistic", "glmm_binomial"),
        ("mixed.poisson", "glmm_poisson"),
        ("mixed.negative_binomial", "glmm_nb"),
    ] {
        let kind = format!("yssbi.statistics.{suffix}");
        let case = &fixture["cases"][name];
        let mut document = GraphDocument::default();
        let source = node(&mut document, "yssbi.constant.get", serde_json::json!({}));
        let labels = fixture["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| format!("subject-{}", g.as_u64().unwrap()))
            .collect::<Vec<_>>();
        let mut table = serde_json::json!({"response":case["y"],"x":fixture["x"][0],"g":labels});
        if matches!(name, "nested" | "crossed") {
            table["h"] = case["second_groups"].clone();
        }
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
        for (key, column, index) in std::iter::once(("response", "response", 0))
            .chain(std::iter::once(("predictors", "x", 0)))
            .chain(std::iter::once(("groups", "g", 0)))
            .chain(matches!(name, "nested" | "crossed").then_some(("groups", "h", 1)))
            .chain((name == "random_slope").then_some(("random_predictors", "x", 0)))
        {
            let from = selector(&mut document, source, column);
            let address = if key == "response" {
                PortAddress::declared(model, key.parse().unwrap())
            } else {
                let a = PortAddress::instance(model, key.parse().unwrap(), PortInstanceId::new());
                document.port_bindings.insert(
                    a.clone(),
                    DynamicPortBinding::UserCreated {
                        order: OrderKey::new(index.to_string()),
                    },
                );
                a
            };
            connect(&mut document, from, "series", address);
        }
        let result = execute(&document, &kind).unwrap_or_else(|e| panic!("{suffix}: {e:?}"));
        let RuntimeValue::Record(report) = result else {
            panic!("structured result")
        };
        assert_eq!(
            report["observations"],
            RuntimeValue::from(TabularScalar::Integer(144))
        );
        let RuntimeValue::List(coefficients) = &report["coefficients"] else {
            panic!("coefficients")
        };
        for (i, c) in coefficients.iter().enumerate() {
            let RuntimeValue::Record(c) = c else {
                panic!("coefficient")
            };
            let RuntimeValue::Scalar(TabularScalar::Float64(estimate)) = &c["estimate"] else {
                panic!("estimate")
            };
            let expected = case["coefficients"][i].as_f64().unwrap();
            assert!((estimate.as_f64() - expected).abs() < 3e-4, "{suffix}: {i}");
        }
        let RuntimeValue::List(groupings) = &report["group_labels"] else {
            panic!("group labels")
        };
        let RuntimeValue::List(first) = &groupings[0] else {
            panic!("first grouping")
        };
        assert_eq!(
            first[0],
            RuntimeValue::from(TabularScalar::String("subject-0".into()))
        );
    }
}

#[test]
fn longitudinal_rejects_independent_and_mixed_row_domains_before_fitting() {
    use arrow::{
        array::Float64Array,
        datatypes::{DataType, Field, Schema},
        record_batch::RecordBatch,
    };
    use yss_node_kernel::{
        KernelControl, KernelError, KernelId, KernelInvocation, KernelOutputSpec, KernelRegistry,
    };
    let relations: Arc<dyn yss_relational_contract::RelationFactory> =
        yss_database_runtime::dataset_query_engine().unwrap();
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    let mut inv = KernelInvocation {
        relations: &relations,
        inputs: &[],
        input_keys: &["response", "groups"],
        parameters: [
            ("constant", RuntimeValue::from(TabularScalar::Bool(true))),
            (
                "max_iterations",
                RuntimeValue::from(TabularScalar::Integer(500)),
            ),
            ("tolerance", RuntimeValue::float64(1e-7).unwrap()),
            (
                "mixed_estimation",
                RuntimeValue::from(TabularScalar::String("reml".into())),
            ),
        ]
        .into_iter()
        .map(|(key, value)| {
            (
                yss_node_kernel::KernelParameterKey::new(key.into()).unwrap(),
                std::borrow::Cow::Owned(value),
            )
        })
        .collect(),
        outputs: &outputs,
        control: &control,
    };
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![Field::new("v", DataType::Float64, false)])),
        vec![Arc::new(Float64Array::from(vec![1., 2., 3., 4.]))],
    )
    .unwrap();
    let relation1 = relations
        .clone()
        .materialize(batch.clone(), &inv.relation_control())
        .unwrap();
    let relation2 = relations
        .clone()
        .materialize(batch, &inv.relation_control())
        .unwrap();
    let first = RuntimeValue::Series(relation1.select_series("v").unwrap());
    let second = RuntimeValue::Series(relation2.select_series("v").unwrap());
    let materialized = RuntimeValue::List(vec![RuntimeValue::float64(1.).unwrap(); 4].into());
    let independent = [first.clone(), second];
    let mixed = [first, materialized];
    for columns in [&independent[..], &mixed[..]] {
        inv.inputs = columns;
        let result = KernelRegistry::default().execute(
            &KernelId::new("yssbi.statistics.mixed.random_intercept".into()).unwrap(),
            &inv,
        );
        assert!(
            matches!(result, Err(KernelError::UnalignedSeries)),
            "{result:?}"
        );
    }
}
