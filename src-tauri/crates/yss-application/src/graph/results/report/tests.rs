use std::sync::Arc;
use std::time::{Duration, Instant};
use yss_data_contract::TabularScalar;

use super::*;
use crate::session::{ApplicationSessionEpoch, ApplicationSessionSlot};
use std::collections::BTreeMap;
use yss_data_contract::{DataValue, ValueType};
use yss_graph_document::{
    DocumentConnection, DocumentNode, GraphConstant, GraphDocument, GraphResourcePath, NodeId,
    NodePosition, ParameterValues, PortAddress,
};
use yss_graph_editor::{EditorGraphMutation, PortPlacement};
use yss_graph_execution::identity::ExecutionSessionId;
use yss_graph_execution::plan::{
    PlanBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint,
};
use yss_graph_execution::resource_preparation::RunResourceBindings;
use yss_graph_execution::state::RunExecutionControl;
use yss_graph_resource_contract::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};

pub(crate) fn fixture(
    n: usize,
) -> (
    ApplicationState,
    ResultReference,
    Arc<LinearRegressionResult>,
) {
    fixture_with_method(n, "OLS")
}

fn fixture_with_method(
    n: usize,
    method: &str,
) -> (
    ApplicationState,
    ResultReference,
    Arc<LinearRegressionResult>,
) {
    fixture_with_options(n, method, LinearSummaryOptions::default())
}

fn fixture_with_options(
    n: usize,
    method: &str,
    summary_options: LinearSummaryOptions,
) -> (
    ApplicationState,
    ResultReference,
    Arc<LinearRegressionResult>,
) {
    let candidate = crate::session::build_current_project_candidate(
        ApplicationSessionEpoch::INITIAL,
        Arc::new(yss_project::ProjectState::new()),
        [],
        &crate::session::NodeComponents::builtins().unwrap(),
    )
    .unwrap();
    let app = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(
        crate::session::NodeComponents::builtins().unwrap(),
    )));
    app.install_candidate(candidate).unwrap();
    let captured = app.capture_session().unwrap();
    let session =
        PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into());
    let runtime = captured.execution();
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
        template_key: "predictors".parse().unwrap(),
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
        (
            response,
            PortAddress::declared(fit, "response".parse().unwrap()),
        ),
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
    let catalog = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let analysis = captured.graph().resolve_graph_document(
        &graph,
        &document,
        &yss_graph_analysis_contract::GraphAnalysisBasis {
            registry_fingerprint: yss_node_registry::RegistryFingerprint::from_bytes(
                captured.graph().registry_fingerprint(),
            ),
            kernel_fingerprint: runtime.kernels().fingerprint().as_bytes(),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        },
        &catalog,
        &[],
        "en-US",
    );
    let basis = PlanBasis::new(
        session.clone(),
        PlanRegistryFingerprint::from_bytes([0; 32]),
        yss_node_kernel::KernelRegistry::default().fingerprint(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let package = runtime
        .prepare_graph_package(&graph, &analysis, basis)
        .unwrap();
    let plan = runtime
        .prepare_package(package, runtime.generation())
        .unwrap();
    let execution = runtime
        .execute_prepared_handoff(
            &plan,
            RunResourceBindings::new(session, [], []),
            captured.resource_provider_factory(),
            &RunExecutionControl::with_cancellation(
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
                Instant::now() + Duration::from_secs(30),
            ),
            yss_graph_execution::state::ExecutionResultRequest {
                demand: &PlanExecutionDemand::Default,
                basis: None,
            },
            |_| {},
        )
        .unwrap();
    assert!(runtime.publish_committed_results(execution.handoff()));
    runtime.finalize_run_success(execution.run_id()).unwrap();
    let reports = execution
        .handoff()
        .results()
        .iter()
        .filter_map(|entry| {
            if let RuntimeValue::LinearRegression(result) = entry.value().value() {
                Some((entry.result_id(), result.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(reports.len(), 3);
    assert!(Arc::ptr_eq(&reports[0].1.model, &reports[2].1.model));
    assert!(Arc::ptr_eq(&reports[0].1.model, &reports[1].1.model));
    let fit = reports
        .iter()
        .find(|(_, value)| value.summary.is_none())
        .unwrap();
    let reference = ResultReference {
        execution_session_id: captured.execution_session_id(),
        result_id: reports
            .iter()
            .find(|(_, value)| value.summary.is_some())
            .unwrap()
            .0,
    };
    (app, reference, fit.1.model.clone())
}

fn overview(
    app: &ApplicationState,
    reference: ResultReference,
) -> Box<LinearRegressionReportProjection> {
    let super::super::ResultValueProjection::LinearReport(report) =
        app.query_result_projection(reference).unwrap().unwrap()
    else {
        panic!("expected an executed Summary");
    };
    report
}

#[test]
fn summary_selection_limits_pages_analyses_and_layout_bindings() {
    let options = LinearSummaryOptions {
        equation: false,
        coefficient_table: false,
        acf_pacf: true,
        acf_max_lag: 3,
        ..Default::default()
    };
    let (app, reference, _) = fixture_with_options(40, "OLS", options.clone());
    let projection = overview(&app, reference);
    assert_eq!(projection.summary, options);
    assert!(matches!(
        app.query_result_table(reference, ResultTablePart::Coefficients, 0, 10),
        Err(ReportQueryError::InvalidRequest)
    ));
    assert!(matches!(
        app.query_result_table(reference, ResultTablePart::Observations, 0, 10),
        Err(ReportQueryError::InvalidRequest)
    ));
    assert!(matches!(
        app.analyze_result(reference, ResultAnalysisRequest::AcfPacf),
        Ok(ResultAnalysisProjection::AcfPacf(_))
    ));
    assert!(matches!(
        app.analyze_result(reference, ResultAnalysisRequest::SerialTests),
        Err(ReportQueryError::InvalidRequest)
    ));
    assert!(matches!(
        app.analyze_result(reference, ResultAnalysisRequest::Hypothesis),
        Err(ReportQueryError::InvalidRequest)
    ));
    use yss_ui_contract::*;
    let session = app.capture_session().unwrap();
    let fit = session.execution().query_graph_results("events/report.yssbi-event", 10)
        .into_iter().find(|result| matches!(result.value().value(), RuntimeValue::LinearRegression(model) if model.summary.is_none())).unwrap();
    let fit_reference = fit.provenance().reference();
    let model = crate::result_encoding::query_result_json(&app, fit_reference)
        .unwrap()
        .unwrap();
    assert_eq!(model["num_observation"], 40);
    assert!(model.get("summary").is_none());
    assert!(model.get("coefficients").is_none());
    assert!(matches!(
        app.query_result_table(fit_reference, ResultTablePart::Coefficients, 0, 10),
        Err(ReportQueryError::WrongKind)
    ));
    assert!(matches!(
        app.analyze_result(fit_reference, ResultAnalysisRequest::AcfPacf),
        Err(ReportQueryError::WrongKind)
    ));
    assert!(matches!(
        app.inspect_ui(
            session.project_instance_id(),
            InspectUiRequest::Page {
                source: UiSource {
                    execution_session_id: fit_reference.execution_session_id.as_uuid().to_string(),
                    result_id: fit_reference.result_id.get().to_string()
                }
            }
        ),
        Err(crate::presentation::UiError::Unavailable)
    ));
    let source = UiSource {
        execution_session_id: reference.execution_session_id.as_uuid().to_string(),
        result_id: reference.result_id.get().to_string(),
    };
    let UiInspection::Page { page } = app
        .inspect_ui(
            session.project_instance_id(),
            InspectUiRequest::Page {
                source: source.clone(),
            },
        )
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        page.spec.elements["report"].children,
        ["modelSummary", "anova", "acfPacf"]
    );
    let mut spec = page.spec;
    spec.elements
        .get_mut("report")
        .unwrap()
        .children
        .push("extra".into());
    spec.elements.insert(
        "extra".into(),
        UiElement {
            component: UiComponent::Analysis {
                binding: "serialTests".into(),
            },
            visible: true,
            children: vec![],
        },
    );
    assert!(matches!(
        app.update_ui(
            session.project_instance_id(),
            UpdateUiRequest {
                source,
                base_revision: 1,
                action: UiAction::Replace { spec }
            }
        ),
        Err(crate::presentation::UiError::Invalid)
    ));
}

#[test]
fn parameter_catalog_is_independent_of_coefficient_pages() {
    let (_, reference, fit) = fixture(10);
    let mut model = (*fit).clone();
    model.report.coefficients = (0..201)
        .map(|index| {
            let mut coefficient = fit.report.coefficients[0].clone();
            coefficient.variable = format!("x{index}");
            coefficient
        })
        .collect();
    let overview = crate::result_encoding::report_to_json(report_projection(
        reference,
        &model,
        &LinearSummaryOptions::default(),
    ));
    assert_eq!(overview["paramNames"].as_array().unwrap().len(), 201);
    assert_eq!(overview["paramNames"][200], "x200");
    assert_eq!(overview["coefficients"]["rowCount"], 201);
}

#[test]
fn large_report_reads_bounded_views_and_runs_tests_on_the_complete_fit() {
    let (app, reference, fit) = fixture_with_options(
        53_940,
        "OLS",
        LinearSummaryOptions {
            observations: true,
            residual_plot: true,
            acf_pacf: true,
            acf_max_lag: 8,
            serial_tests: true,
            serial_lags: 4,
            hypothesis_test: true,
            ..Default::default()
        },
    );
    let stored = app
        .capture_session()
        .unwrap()
        .execution()
        .query_result(reference.result_id)
        .unwrap();
    let RuntimeValue::LinearRegression(native) = stored.value().value() else {
        panic!()
    };
    let analyses = native.summary.as_ref().unwrap();
    let lease = uuid::Uuid::new_v4();
    app.retain_result(reference, lease, "report", None).unwrap();
    app.capture_session()
        .unwrap()
        .execution()
        .invalidate_graph_results("events/report.yssbi-event");
    let overview = overview(&app, reference);
    assert_eq!(overview.observation_count, 53_940);
    assert_eq!(overview.coefficient_count, 2);
    let page = app
        .query_result_table(reference, ResultTablePart::Observations, 53_930, 200)
        .unwrap();
    assert_eq!(page.values.len(), 10);
    assert!(!page.has_more);
    assert_eq!(
        page.values[0],
        RuntimeValue::List(std::sync::Arc::from([
            RuntimeValue::Scalar(TabularScalar::Unsigned(53_931)),
            RuntimeValue::float64(fit.fitted[53_930]).unwrap(),
            RuntimeValue::float64(fit.residuals[53_930]).unwrap(),
        ]))
    );
    let ResultAnalysisProjection::ResidualPlot(plot) = app
        .analyze_result(
            reference,
            ResultAnalysisRequest::ResidualPlot {
                max_points: 32,
                x_range: None,
            },
        )
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(plot.points.len(), 32);
    assert!(plot.sampled);
    assert_eq!(plot.points.first().unwrap().observation, 1);
    assert_eq!(plot.points.last().unwrap().observation, 53_940);
    for point in &plot.points {
        assert_eq!(point.x, fit.fitted[point.observation - 1]);
        assert_eq!(point.y, fit.residuals[point.observation - 1]);
    }
    let ResultAnalysisProjection::AcfPacf(acf) = app
        .analyze_result(reference, ResultAnalysisRequest::AcfPacf)
        .unwrap()
    else {
        panic!()
    };
    let expected = analyses.acf.as_ref().unwrap();
    assert_eq!(&acf, expected.as_ref());
    assert_eq!(acf.n, 53_940);
    let ResultAnalysisProjection::SerialTests(serial) = app
        .analyze_result(reference, ResultAnalysisRequest::SerialTests)
        .unwrap()
    else {
        panic!()
    };
    let expected = analyses.serial.as_ref().unwrap();
    assert_eq!(serial.dw.d, expected.dw.d);
    assert_eq!(serial.q.unwrap().stat, expected.q.as_ref().unwrap().stat);
    assert_eq!(serial.bg.unwrap().stat, expected.bg.as_ref().unwrap().stat);
    let ResultAnalysisProjection::Hypothesis(test) = app
        .analyze_result(reference, ResultAnalysisRequest::Hypothesis)
        .unwrap()
    else {
        panic!()
    };
    assert!((test.stat - fit.report.coefficients[1].t_value).abs() < 1e-8);
    assert_eq!(test.df2, 53_938);
    app.release_result_lease(lease, "report").unwrap();
    assert!(matches!(app.query_result_projection(reference), Ok(None)));
}

#[test]
fn references_reject_other_sessions_and_in_flight_results_after_invalidation() {
    for fail in [false, true] {
        let (app, reference, _) = fixture(30);
        let foreign = ResultReference {
            execution_session_id: ExecutionSessionId::new(uuid::Uuid::new_v4()),
            ..reference
        };
        assert!(matches!(
            app.query_result_table(foreign, ResultTablePart::Coefficients, 0, 1),
            Err(ReportQueryError::Stale)
        ));
        let outcome = app.with_linear_regression_summary(reference, |_, _| {
            app.capture_session()
                .unwrap()
                .execution()
                .invalidate_graph_results("events/report.yssbi-event");
            if fail {
                Err(ReportQueryError::InvalidRequest)
            } else {
                Ok(())
            }
        });
        assert!(matches!(outcome, Err(ReportQueryError::Unavailable)));
        assert!(matches!(
            app.query_result_table(reference, ResultTablePart::Observations, 0, 1),
            Err(ReportQueryError::Unavailable)
        ));
    }
}

#[test]
fn weighted_graph_inputs_reach_the_shared_report_query() {
    let (wls_app, wls_reference, wls) = fixture_with_method(8, "WLS");
    let (gls_app, gls_reference, gls) = fixture_with_method(8, "GLS");
    for (app, reference, model, method) in [
        (&wls_app, wls_reference, &wls, "WLS"),
        (&gls_app, gls_reference, &gls, "GLS"),
    ] {
        let overview = overview(app, reference);
        assert_eq!(overview.model.model_type, method);
        assert_eq!(overview.observation_count, 8);
        assert!(model.constant);
    }
    for (a, b) in wls.coefficients.iter().zip(&gls.coefficients) {
        assert!((a - b).abs() < 1e-10);
    }
    assert!(
        (wls.report.model_basic_info.r_squared - gls.report.model_basic_info.r_squared).abs()
            < 1e-10
    );
}
