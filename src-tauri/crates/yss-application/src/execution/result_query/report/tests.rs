use super::*;
use crate::execution::{ApplicationSessionEpoch, ApplicationSessionSlot};
use std::collections::BTreeMap;
use yss_data_contract::{DataSeriesValue, DataType, DataValue};
use yss_execution::identity::ExecutionSessionId;
use yss_execution::plan::{
    PlanCompilationBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint,
};
use yss_execution::resource_preparation::RunResourceBindings;
use yss_execution::state::RunExecutionControl;
use yss_graph_analysis_contract::CompileId;
use yss_graph_compiler::{GraphCompilationInput, compile};
use yss_graph_document::{
    DocumentConnection, DocumentNode, GraphConstant, GraphDocument, GraphResourcePath, NodeId,
    NodePosition, ParameterValues, PortAddress,
};
use yss_graph_editor::{EditorGraphMutation, PortPlacement};
use yss_graph_resource_contract::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};

fn fixture(n: usize) -> (ApplicationState, ResultReference, Arc<OlsResult>) {
    let candidate = crate::execution::session_factory::build_current_project_candidate(
        ApplicationSessionEpoch::INITIAL,
        Arc::new(yss_project::ProjectState::new()),
        [],
        Arc::new(yss_sci_runtime::SciRuntimeBackend::new()),
    )
    .unwrap();
    let app = ApplicationState::from_composition(
        Arc::new(ApplicationSessionSlot::new()),
        Arc::new(yss_sci_runtime::SciRuntimeBackend::new()),
    );
    app.install_candidate(candidate).unwrap();
    let captured = app.capture_session().unwrap();
    let session =
        PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into());
    let runtime = captured.execution();
    let graph = GraphResourcePath::new("events/report.yssbi-event").unwrap();
    let builtins = yss_graph_catalog::build_builtin_node_system().unwrap();
    let mut document = GraphDocument::default();
    let [response, predictor, summary] = std::array::from_fn(|_| NodeId::new());
    for (id, node_type) in [
        (response, "yssbi.constant.get"),
        (predictor, "yssbi.constant.get"),
        (summary, "yssbi.statistics.ols.summary"),
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
            data_type: DataType::DataSeries(Box::new(DataType::Float64)),
            data_value: DataValue::DataSeries(DataSeriesValue::with_element_type(
                serde_json::json!({ "value": values }).to_string(),
                DataType::Float64,
            )),
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
        node_id: summary,
        template_key: "predictors".parse().unwrap(),
        placement: PortPlacement::Append,
    }
    .into_patch(&graph, &document, &builtins.registry)
    .unwrap();
    yss_graph_document_edit::apply_graph_document_patch(&mut document, &patch).unwrap();
    let input = document
        .port_bindings
        .keys()
        .find(|port| port.node_id == summary)
        .unwrap()
        .clone();
    for (source, input) in [
        (
            response,
            PortAddress::declared(summary, "response".parse().unwrap()),
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
    let catalog = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let semantic =
        yss_graph_analysis::resolve_graph_semantics(&document, &builtins.registry, &catalog);
    let package = compile(GraphCompilationInput::new(
        semantic
            .ready()
            .unwrap_or_else(|| panic!("{:?}", semantic.diagnostics())),
        graph,
        CompileId::new(1),
    ))
    .unwrap();
    let basis = PlanCompilationBasis::new(
        session.clone(),
        PlanRegistryFingerprint::from_bytes([0; 32]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let package = crate::graph_contracts::execution_package_from_graph(package, basis).unwrap();
    let plan = runtime
        .prepare_compiled_package(package, runtime.generation())
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
            &PlanExecutionDemand::Default,
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
            if let StoredResult::Runtime(RuntimeValue::Ols(result)) = entry.value().value() {
                Some((entry.result_id(), result.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(reports.len(), 2);
    assert!(Arc::ptr_eq(&reports[0].1, &reports[1].1));
    let reference = ResultReference {
        execution_session_id: captured.execution_session_id(),
        result_id: reports[0].0,
    };
    (app, reference, reports[0].1.clone())
}

#[test]
fn large_report_reads_bounded_views_and_runs_tests_on_the_complete_fit() {
    let (app, reference, fit) = fixture(53_940);
    let lease = uuid::Uuid::new_v4();
    app.retain_result(reference, lease, "report", None).unwrap();
    app.capture_session()
        .unwrap()
        .execution()
        .invalidate_graph_results("events/report.yssbi-event");
    let overview = app.query_ols_report(reference).unwrap();
    assert_eq!(overview.observation_count, 53_940);
    assert_eq!(overview.coefficient_count, 2);
    let page = app
        .query_result_table(reference, ResultTablePart::Observations, 53_930, 200)
        .unwrap();
    assert_eq!(page.values.len(), 10);
    assert!(!page.has_more);
    assert_eq!(
        page.values[0],
        RuntimeValue::List(Box::new([
            RuntimeValue::Unsigned(53_931),
            RuntimeValue::Decimal(fit.fitted[53_930]),
            RuntimeValue::Decimal(fit.residuals[53_930]),
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
        .analyze_result(reference, ResultAnalysisRequest::AcfPacf { max_lag: 8 })
        .unwrap()
    else {
        panic!()
    };
    let expected = crate::statistics::compute_acf_pacf(&app, fit.residuals.clone(), 8).unwrap();
    assert_eq!(acf, expected);
    assert_eq!(acf.n, 53_940);
    let ResultAnalysisProjection::SerialTests(serial) = app
        .analyze_result(
            reference,
            ResultAnalysisRequest::SerialTests {
                lags: 4,
                bg_nomiss0: true,
            },
        )
        .unwrap()
    else {
        panic!()
    };
    let expected = crate::statistics::compute_serial_tests(SerialTestsRequest {
        residuals: fit.residuals.clone(),
        lags: 4,
        bg_nomiss0: true,
        exog: Some(
            (0..fit.residuals.len())
                .map(|row| fit.design.iter().map(|column| column[row]).collect())
                .collect(),
        ),
    })
    .unwrap();
    assert_eq!(serial.dw.d, expected.dw.d);
    assert_eq!(serial.q.unwrap().stat, expected.q.unwrap().stat);
    assert_eq!(serial.bg.unwrap().stat, expected.bg.unwrap().stat);
    let ResultAnalysisProjection::Hypothesis(test) = app
        .analyze_result(
            reference,
            ResultAnalysisRequest::Hypothesis {
                hypothesis: "x1 = 0".into(),
            },
        )
        .unwrap()
    else {
        panic!()
    };
    assert!((test.stat - fit.report.coefficients[1].t_value).abs() < 1e-8);
    assert_eq!(test.df2, 53_938);
    app.release_result_lease(lease, "report").unwrap();
    assert!(matches!(
        app.query_ols_report(reference),
        Err(ReportQueryError::Unavailable)
    ));
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
        let outcome = app.with_ols_result(reference, |_| {
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
