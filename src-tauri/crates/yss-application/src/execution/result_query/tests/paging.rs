use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use yss_execution::plan::{
    PlanCompilationBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint,
    PlanResourceId, PlanResourceObservedState, PlanResourceRequirement, PlanResourceVersion,
    ResourceAccess, ResourceKind,
};
use yss_execution::resource_preparation::{RunResourceBinding, RunResourceBindings};
use yss_execution::state::RunExecutionControl;
use yss_execution::value::RuntimeValue;
use yss_graph_analysis_contract::CompileId;
use yss_graph_compiler::{GraphCompilationInput, compile};
use yss_graph_document::{
    DocumentNode, GraphDocument, GraphResourcePath, NodeId, NodePosition, ParameterValues,
};
use yss_graph_resource_contract::{
    ColumnSchema, DataSchema, GraphResourceId, ResourceCatalogFingerprint, ResourceCatalogSnapshot,
};
use yss_relational_contract::{
    RelationBatchStream, RelationBinding, RelationColumn, RelationControl, RelationError,
    RelationExecutor, RelationFuture, RelationHandle, RelationPage, RelationPlan,
    RelationPredicate, SeriesHandle,
};
use yss_tabular_contract::{TabularColumn, TabularColumnName, TabularScalar, TabularSnapshot};

use crate::execution::{ApplicationSessionEpoch, ApplicationSessionSlot, ApplicationState};

struct DelayedPage {
    binding: RelationBinding,
    entered: mpsc::SyncSender<()>,
    resume: Mutex<mpsc::Receiver<()>>,
    fail: bool,
}

impl RelationPlan for DelayedPage {
    fn binding(&self) -> &RelationBinding {
        &self.binding
    }
    fn schema(&self) -> SchemaRef {
        Arc::new(Schema::new(vec![Field::new("x", DataType::Float64, true)]))
    }
    fn project(&self, _: &[Box<str>]) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidPlan)
    }
    fn filter(&self, _: &RelationPredicate) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidPlan)
    }
    fn limit(&self, _: usize, _: usize) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidPlan)
    }
    fn rename(&self, _: &str, _: &str) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidPlan)
    }
    fn stream(&self, _: RelationControl) -> RelationFuture<'_, RelationBatchStream> {
        Box::pin(async { Err(RelationError::InvalidPlan) })
    }
}

impl RelationExecutor for DelayedPage {
    fn numeric_columns(
        &self,
        _: &[SeriesHandle],
        _: &RelationControl,
    ) -> Result<Vec<Vec<f64>>, RelationError> {
        Err(RelationError::InvalidPlan)
    }
    fn page(
        &self,
        _: &RelationHandle,
        _: usize,
        _: usize,
        _: &RelationControl,
    ) -> Result<RelationPage, RelationError> {
        self.entered.send(()).unwrap();
        self.resume
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(10))
            .unwrap();
        if self.fail {
            return Err(RelationError::QueryFailed);
        }
        Ok(RelationPage {
            data: TabularSnapshot::try_from_columns(Box::new([TabularColumn::new(
                TabularColumnName::try_from("x").unwrap(),
                Box::new([TabularScalar::Integer(1)]),
            )]))
            .unwrap(),
            row_count: 1,
            columns: Box::new([RelationColumn {
                name: "x".into(),
                data_type: "Float64".into(),
            }]),
            has_more: false,
        })
    }
}

#[test]
fn invalidation_discards_in_flight_page_success_and_failure() {
    for fail in [false, true] {
        let project = Arc::new(yss_project::ProjectState::new());
        let candidate = crate::execution::session_factory::build_current_project_candidate(
            ApplicationSessionEpoch::INITIAL,
            project,
            [],
            Arc::new(yss_sci_runtime::SciRuntimeBackend::new()),
        )
        .unwrap();
        let app = ApplicationState::new(Arc::new(ApplicationSessionSlot::new()));
        app.install_candidate(candidate).unwrap();
        let captured = app.capture_session().unwrap();
        let session =
            PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into());
        let runtime = captured.execution();
        let graph = GraphResourcePath::new("events/paged.yssbi-event").unwrap();
        let id = NodeId::new();
        let mut document = GraphDocument::default();
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: "yssbi.dataframe.source.get".parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::from([(
                    "dataframe".parse().unwrap(),
                    serde_json::json!("data"),
                )]),
                user_label: None,
            },
        );
        let catalog = ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::from([(
                GraphResourceId::new("data"),
                DataSchema {
                    columns: vec![ColumnSchema {
                        name: "x".into(),
                        data_type: yss_data_contract::DataType::Float64,
                    }],
                },
            )]),
            ResourceCatalogFingerprint::from_bytes([0; 32]),
        );
        let builtin = yss_graph_catalog::build_builtin_node_system().unwrap();
        let semantic =
            yss_graph_analysis::resolve_graph_semantics(&document, &builtin.registry, &catalog);
        let package = compile(GraphCompilationInput::new(
            semantic.ready().unwrap(),
            graph.clone(),
            CompileId::new(1),
        ))
        .unwrap();
        let resource = PlanResourceId::from_existing("data".into());
        let version = PlanResourceVersion::from_existing("7".into());
        let basis = PlanCompilationBasis::new(
            session.clone(),
            PlanRegistryFingerprint::from_bytes([0; 32]),
            BTreeMap::from([(resource.clone(), version.clone())]),
            BTreeMap::from([(
                resource.clone(),
                PlanResourceObservedState::Present(version.clone()),
            )]),
        );
        let package = crate::graph_contracts::execution_package_from_graph(package, basis).unwrap();
        let plan = runtime
            .prepare_compiled_package(package, runtime.generation())
            .unwrap();
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = mpsc::sync_channel(1);
        let reader = Arc::new(DelayedPage {
            binding: RelationBinding {
                project_session: session.as_str().into(),
                dataset: yss_database_contract::DatabaseId::from_existing("data".into()),
                snapshot: "snapshot-7".into(),
                revision: 7,
            },
            entered: entered_tx,
            resume: Mutex::new(resume_rx),
            fail,
        });
        let relation = RelationHandle::new(reader.clone(), reader);
        let requirement = PlanResourceRequirement::new(
            resource,
            ResourceKind::DataFrame,
            ResourceAccess::Shared,
            false,
        );
        let execution = runtime
            .execute_prepared_handoff(
                &plan,
                RunResourceBindings::new(
                    session,
                    [requirement.clone()],
                    [RunResourceBinding::new(
                        requirement,
                        version,
                        RuntimeValue::Relation(relation),
                    )],
                ),
                captured.resource_provider_factory(),
                &RunExecutionControl::with_cancellation(
                    Arc::new(std::sync::atomic::AtomicBool::new(false)),
                    Instant::now() + Duration::from_secs(10),
                ),
                &PlanExecutionDemand::Default,
                |_| {},
            )
            .unwrap();
        assert!(runtime.publish_committed_results(execution.handoff()));
        let result_id = execution.handoff().results()[0].result_id();
        let worker = std::thread::spawn(move || app.query_result_page(result_id, 0, 1));
        entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        runtime.invalidate_graph_results(graph.as_str());
        resume_tx.send(()).unwrap();
        assert!(worker.join().unwrap().unwrap().is_none());
    }
}
