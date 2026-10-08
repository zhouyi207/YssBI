use std::sync::Arc;
use std::time::{Duration, Instant};
use yss_data_contract::TabularScalar;

use super::*;
use crate::session::{ApplicationSessionEpoch, ApplicationSessionSlot};
use std::collections::BTreeMap;
use yss_graph_execution::identity::ExecutionSessionId;
use yss_graph_execution::plan::{
    PlanBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint,
};
use yss_graph_execution::resource_preparation::RunResourceBindings;
use yss_graph_execution::state::RunExecutionControl;
use yss_graph_resource_contract::ResourceCatalogSnapshot;

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

pub(crate) fn fixture_with_options(
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
    let (graph, document) = super::fixtures::linear_document(n, method, summary_options);
    let catalog = ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new());
    let analysis = captured.graph().resolve_graph_document(
        &graph,
        &Arc::new(document.clone()),
        &yss_graph_analysis_contract::GraphAnalysisBasis {
            registry_fingerprint: yss_node_registry::RegistryFingerprint::from_bytes(
                captured.graph().registry_fingerprint(),
            ),
            kernel_fingerprint: runtime.kernels().fingerprint().as_bytes(),
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
    );
    let package = runtime
        .prepare_graph_package(
            &graph,
            &analysis,
            basis,
            &yss_graph_execution::graph_preparation::GraphExecutionScope::all(
                analysis.semantic_snapshot(),
            ),
        )
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
    assert_eq!(reports.len(), 2);
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
fn summary_selection_limits_pages_and_analyses() {
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
        app.query_result_coefficients(reference, 0, 10),
        Err(ReportQueryError::InvalidRequest)
    ));
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
    let session = app.capture_session().unwrap();
    let fit = session.execution().query_graph_results("events/report.yssbi-event", 10)
        .into_iter().find(|result| matches!(result.value().value(), RuntimeValue::LinearRegression(model) if model.summary.is_none())).unwrap();
    let fit_reference = fit.provenance().reference();
    assert!(matches!(
        app.query_result_coefficients(fit_reference, 0, 10),
        Err(ReportQueryError::WrongKind)
    ));
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
    let tail = coefficients::page(&model.report.coefficients, 200, 200).unwrap();
    assert_eq!(tail.offset, 200);
    assert_eq!(tail.total_count, 201);
    assert_eq!(tail.coefficients.len(), 1);
    assert_eq!(tail.coefficients[0].variable, "x200");

    assert!(report_projection(reference, &model, &LinearSummaryOptions::default()).constant);
    model.constant = false;
    model.report.coefficients[0].variable = "const".into();
    let projection = report_projection(reference, &model, &LinearSummaryOptions::default());
    assert!(!projection.constant);
    assert_eq!(projection.param_names[0], "const");

    model.report.coefficients[200].p_value = f64::NAN;
    assert!(coefficients::page(&model.report.coefficients, 0, 200).is_ok());
    assert!(matches!(
        coefficients::page(&model.report.coefficients, 200, 200),
        Err(ReportQueryError::UnrepresentableValue)
    ));
}

#[test]
fn typed_coefficient_pages_keep_values_bounds_and_lease_identity() {
    let (app, reference, fit) = fixture(30);
    let lease = uuid::Uuid::new_v4();
    app.retain_result(reference, lease, "coefficient-report", None)
        .unwrap();
    app.capture_session()
        .unwrap()
        .execution()
        .invalidate_graph_results("events/report.yssbi-event");
    let page = app.query_result_coefficients(reference, 1, 200).unwrap();
    assert_eq!(page.offset, 1);
    assert_eq!(page.total_count, fit.report.coefficients.len());
    assert_eq!(page.coefficients, fit.report.coefficients[1..]);
    let empty = app.query_result_coefficients(reference, 999, 200).unwrap();
    assert_eq!(empty.offset, page.total_count);
    assert!(empty.coefficients.is_empty());
    for (offset, limit) in [(0, 0), (0, MAX_RESULT_PAGE_ROWS + 1), (usize::MAX, 1)] {
        assert!(matches!(
            app.query_result_coefficients(reference, offset, limit),
            Err(ReportQueryError::InvalidRequest)
        ));
    }
    let foreign = ResultReference {
        execution_session_id: ExecutionSessionId::new(uuid::Uuid::new_v4()),
        ..reference
    };
    assert!(matches!(
        app.query_result_coefficients(foreign, 0, 1),
        Err(ReportQueryError::Stale)
    ));
    app.release_result_lease(lease, "coefficient-report")
        .unwrap();
    assert!(matches!(
        app.query_result_coefficients(reference, 0, 1),
        Err(ReportQueryError::Unavailable)
    ));
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
                adjacent: false,
                highlight_top_percent: None,
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

#[test]
fn residual_ranges_keep_population_highlights_and_original_observations() {
    let (app, reference, _) = fixture_with_options(
        48,
        "OLS",
        LinearSummaryOptions {
            residual_plot: true,
            ..Default::default()
        },
    );
    let read = |x_range, max_points| {
        let ResultAnalysisProjection::ResidualPlot(value) = app
            .analyze_result(
                reference,
                ResultAnalysisRequest::ResidualPlot {
                    max_points,
                    x_range,
                    adjacent: false,
                    highlight_top_percent: Some(25.),
                },
            )
            .unwrap()
        else {
            panic!("expected residual plot")
        };
        value
    };
    let full = read(None, 100);
    assert_eq!(full.points.iter().filter(|p| p.highlighted).count(), 12);
    let [a, b] = [full.points[12].x, full.points[35].x];
    let range = [a.min(b), a.max(b)];
    let matching = full
        .points
        .iter()
        .filter(|p| p.x >= range[0] && p.x <= range[1])
        .collect::<Vec<_>>();
    assert!(matching.len() > 5);
    let filtered = read(Some(range), 5);
    assert_eq!(filtered.total_count, 48);
    assert_eq!(filtered.matched_count, matching.len());
    assert_eq!(filtered.points.len(), 5);
    assert!(filtered.sampled);
    assert_eq!(
        filtered.points.first().unwrap().observation,
        matching.first().unwrap().observation
    );
    assert_eq!(
        filtered.points.last().unwrap().observation,
        matching.last().unwrap().observation
    );
    for point in &filtered.points {
        let original = matching
            .iter()
            .find(|p| p.observation == point.observation)
            .unwrap();
        assert_eq!(
            (point.x, point.y, point.highlighted),
            (original.x, original.y, original.highlighted)
        );
    }
    let empty = read(Some([b.max(a) + 1000., b.max(a) + 1001.]), 5);
    assert_eq!(empty.matched_count, 0);
    assert!(empty.points.is_empty());
    assert!(!empty.sampled);
}

#[test]
fn diagnostic_queries_return_computed_tests_and_highlight_complete_population() {
    let (app, reference, fit) = fixture_with_options(
        48,
        "OLS",
        LinearSummaryOptions {
            diagnostics: true,
            residual_plot: true,
            ..Default::default()
        },
    );
    let ResultAnalysisProjection::Diagnostics(diagnostics) = app
        .analyze_result(reference, ResultAnalysisRequest::Diagnostics)
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(diagnostics.tests.len(), 6);
    assert!(
        diagnostics
            .tests
            .iter()
            .any(|test| test.name.starts_with("Breusch") && test.value.is_some())
    );
    assert_eq!(diagnostics.leverage_density.len(), 128);
    let ResultAnalysisProjection::ResidualPlot(plot) = app
        .analyze_result(
            reference,
            ResultAnalysisRequest::ResidualPlot {
                max_points: 100,
                x_range: None,
                adjacent: true,
                highlight_top_percent: Some(100.0),
            },
        )
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(plot.matched_count, 47);
    assert_eq!(plot.points[0].observation, 2);
    for point in &plot.points {
        assert_eq!(point.x, fit.residuals[point.observation - 2]);
        assert_eq!(point.y, fit.residuals[point.observation - 1]);
        assert!(point.highlighted);
    }
    assert!(
        app.analyze_result(
            reference,
            ResultAnalysisRequest::ResidualPlot {
                max_points: 100,
                x_range: None,
                adjacent: true,
                highlight_top_percent: Some(101.0)
            }
        )
        .is_err()
    );
    let (unselected, other, _) = fixture(48);
    assert!(
        unselected
            .analyze_result(other, ResultAnalysisRequest::Diagnostics)
            .is_err()
    );
}
