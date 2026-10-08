use super::*;
use crate::{
    graph::editing::GraphActivity,
    session::{ApplicationSessionEpoch, ApplicationSessionSlot, NodeComponents},
};
use yss_graph_document::{GraphResourceKind, NodeId};
use yss_node_kernel::RuntimeValue;
use yss_project::ProjectState;
use yss_project_model::{GraphResourceDocument, ProjectData};

struct Fixture {
    application: ApplicationState,
    reference: ResultReference,
    source: NodeId,
    graph: GraphResourcePath,
    project: ProjectInstanceId,
    _directory: tempfile::TempDir,
    _lease: ResultLease,
}

impl Fixture {
    fn new() -> Self {
        let (graph, document) =
            super::super::fixtures::linear_document(48, "OLS", Default::default());
        let source = document
            .nodes
            .values()
            .find(|node| node.node_type.as_str() == "yssbi.statistics.linear.summary")
            .unwrap()
            .id;
        let mut data = ProjectData::new();
        let mut resource = GraphResourceDocument::new("report", GraphResourceKind::EventGraph);
        resource.document = Arc::new(document);
        data.graphs.insert(graph.clone(), resource);
        let directory = tempfile::tempdir().unwrap();
        yss_project::fixtures::write_project(&data, directory.path().to_str().unwrap()).unwrap();
        let project = Arc::new(ProjectState::new());
        project.activate_project_fixture(directory.path().to_str().unwrap().into(), data);
        let candidate = crate::session::build_current_project_candidate(
            ApplicationSessionEpoch::INITIAL,
            project,
            [],
            &NodeComponents::builtins().unwrap(),
        )
        .unwrap();
        let application = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(
            NodeComponents::builtins().unwrap(),
        )));
        application.install_candidate(candidate).unwrap();
        let project = application
            .capture_session()
            .unwrap()
            .project_instance_id()
            .clone();
        let opened = application
            .open_graph(OpenGraphRequest::new(
                project.clone(),
                graph.clone(),
                0,
                "en-US",
            ))
            .unwrap();
        let receipt = run_graph_with_sink(
            &application,
            RunGraphRequest::new(
                project.clone(),
                graph.clone(),
                opened.document().clone(),
                opened.analysis().semantic_input_hash().to_owned(),
            ),
            |_| true,
        )
        .unwrap();
        let result = receipt
            .results
            .iter()
            .find(|result| {
                result.category
                    == ResultCategory::StatisticalReport(
                        StatisticalReportKind::LinearRegressionSummary,
                    )
            })
            .unwrap();
        let reference = ResultReference {
            execution_session_id: *receipt.identity.execution_session_id(),
            result_id: result.result_id,
        };
        let (lease, _) = application.retain_owned_result(reference).unwrap();
        Self {
            application,
            reference,
            source,
            graph,
            project,
            _directory: directory,
            _lease: lease,
        }
    }

    fn edit(&self, key: &str, value: bool) {
        let opened = self
            .application
            .open_graph(OpenGraphRequest::new(
                self.project.clone(),
                self.graph.clone(),
                0,
                "en-US",
            ))
            .unwrap();
        self.application
            .edit_graph(
                GraphEditRequest {
                    project_instance_id: self.project.clone(),
                    graph_path: self.graph.clone(),
                    version: opened.editing().version,
                    operation_id: OperationId::new(),
                    locale: "en-US".into(),
                },
                EditorGraphMutation::SetParameters {
                    node_id: self.source,
                    parameters: [(key.parse().unwrap(), value.into())].into(),
                },
            )
            .unwrap();
    }

    fn add(
        &self,
        content: LinearSummaryContent,
        cancellation: Arc<AtomicBool>,
    ) -> Result<LinearReportUpdate, ReportAdditionError> {
        self.application.add_linear_report_contents(
            self.reference,
            LinearSummaryAddition {
                contents: [content].into(),
                acf_max_lag: 3,
                serial_lags: 2,
                bg_nomiss0: false,
                hypothesis: "x1 = 0".into(),
            },
            "en-US",
            cancellation,
        )
    }
}

#[test]
fn additions_merge_current_parameters_reuse_fit_and_transfer_only_complete_results() {
    let fixture = Fixture::new();
    let app = &fixture.application;
    let disk = std::fs::read(fixture._directory.path().join(fixture.graph.as_str())).unwrap();
    let before = app.query_result(fixture.reference).unwrap().unwrap();
    let RuntimeValue::LinearRegression(original) = before.value().value() else {
        panic!()
    };
    fixture.edit("coefficient_chart", true);
    let inspections = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = inspections.clone();
    let _events = app.subscribe_graph_activity(&fixture.project, Arc::new(move |activity| {
        if matches!(activity, GraphActivity::Execution(event) if matches!(event.kind(), crate::graph::run::RunApplicationEventKind::ResultInspectionRequested { .. })) {
            observed.fetch_add(1, Ordering::Relaxed);
        }
    })).unwrap();
    let update = fixture
        .add(
            LinearSummaryContent::AcfPacf,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
    let reference = update.report().reference;
    assert_ne!(reference, fixture.reference);
    assert!(update.report().summary.coefficient_chart);
    assert!(update.report().summary.acf_pacf);
    assert_eq!(update.report().summary.acf_max_lag, 3);
    assert!(
        update.report().summary.bg_nomiss0,
        "unselected analysis parameters stay unchanged"
    );
    let after = app.query_result(reference).unwrap().unwrap();
    let RuntimeValue::LinearRegression(result) = after.value().value() else {
        panic!()
    };
    assert!(
        Arc::ptr_eq(&original.model, &result.model),
        "append must reuse the fitted input"
    );
    assert!(result.summary.as_ref().unwrap().acf.is_some());
    assert!(!original.summary.as_ref().unwrap().options.acf_pacf);
    assert_eq!(
        inspections.load(Ordering::Relaxed),
        0,
        "append must not open another result panel"
    );
    let (lease, report) = update.accept(app).unwrap();
    assert_eq!(lease.reference(), report.reference);
    let opened = app
        .open_graph(OpenGraphRequest::new(
            fixture.project.clone(),
            fixture.graph.clone(),
            0,
            "en-US",
        ))
        .unwrap();
    assert!(opened.editing().dirty);
    assert_eq!(
        disk,
        std::fs::read(fixture._directory.path().join(fixture.graph.as_str())).unwrap()
    );
    app.capture_session()
        .unwrap()
        .execution()
        .invalidate_graph_results(fixture.graph.as_str());
    drop(lease);
    assert!(app.query_result(reference).unwrap().is_none());
    assert!(app.query_result(fixture.reference).unwrap().is_some());
}

#[test]
fn cancelled_and_superseded_deliveries_keep_the_original_report_and_release_new_leases() {
    let fixture = Fixture::new();
    let app = &fixture.application;
    let cancellation = Arc::new(AtomicBool::new(true));
    assert!(matches!(
        fixture.add(LinearSummaryContent::Observations, cancellation.clone()),
        Err(ReportAdditionError::Changed)
    ));
    let opened = app
        .open_graph(OpenGraphRequest::new(
            fixture.project.clone(),
            fixture.graph.clone(),
            0,
            "en-US",
        ))
        .unwrap();
    assert!(
        !opened.editing().dirty,
        "cancellation before submission cannot edit the graph"
    );
    cancellation.store(false, Ordering::Release);
    let cancel_run = cancellation.clone();
    let subscription = app.subscribe_graph_activity(&fixture.project, Arc::new(move |activity| {
        if matches!(activity, GraphActivity::Execution(event) if matches!(event.kind(), crate::graph::run::RunApplicationEventKind::RunStarted { .. })) {
            cancel_run.store(true, Ordering::Release);
        }
    })).unwrap();
    let error = fixture
        .add(LinearSummaryContent::Observations, cancellation.clone())
        .err()
        .expect("cancelled execution");
    assert!(
        matches!(
            error,
            ReportAdditionError::Run(ExecutionApplicationError::PreparedExecution(
                yss_graph_execution::state::ExecutePreparedError::Cancelled {
                    phase: yss_graph_execution::error::RunPhase::Execution
                }
            ))
        ),
        "{error:?}"
    );
    assert!(cancellation.load(Ordering::Acquire));
    drop(subscription);
    assert!(app.query_result(fixture.reference).unwrap().is_some());
    let update = fixture
        .add(
            LinearSummaryContent::Observations,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
    let reference = update.report().reference;
    fixture.edit("model_summary", false);
    assert!(matches!(
        update.accept(app),
        Err(ReportAdditionError::Changed)
    ));
    app.capture_session()
        .unwrap()
        .execution()
        .invalidate_graph_results(fixture.graph.as_str());
    assert!(
        app.query_result(reference).unwrap().is_none(),
        "rejected delivery must release its lease"
    );
    assert!(app.query_result(fixture.reference).unwrap().is_some());
}
