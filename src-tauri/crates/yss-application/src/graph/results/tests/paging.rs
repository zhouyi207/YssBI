use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use yss_graph_document::{
    DocumentNode, GraphDocument, GraphResourcePath, NodeId, NodePosition, ParameterValues,
};
use yss_graph_execution::plan::{
    PlanBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint, PlanResourceId,
    PlanResourceObservedState, PlanResourceRequirement, PlanResourceVersion, ResourceAccess,
    ResourceKind,
};
use yss_graph_execution::resource_preparation::{RunResourceBinding, RunResourceBindings};
use yss_graph_execution::state::RunExecutionControl;
use yss_graph_resource_contract::{
    ColumnSchema, DataSchema, GraphResourceId, ResourceCatalogFingerprint, ResourceCatalogSnapshot,
};
use yss_node_kernel::RuntimeValue;
use yss_relational_contract::{
    NumericOperation, NumericType, RelationBatchStream, RelationBinding, RelationColumn,
    RelationControl, RelationError, RelationExecutor, RelationFuture, RelationHandle, RelationPage,
    RelationPlan, RelationPredicate, SeriesHandle, SeriesOperand, SeriesPlan,
};
use yss_tabular_contract::{TabularColumn, TabularColumnName, TabularScalar, TabularSnapshot};

use crate::session::{ApplicationSessionEpoch, ApplicationSessionSlot, ApplicationState};

struct DelayedPage {
    binding: RelationBinding,
    entered: mpsc::SyncSender<()>,
    resume: Mutex<mpsc::Receiver<()>>,
    fail: bool,
}

impl RelationPlan for DelayedPage {
    fn boolean_series(
        &self,
        _: yss_relational_contract::BooleanOperation,
        _: &[yss_relational_contract::BooleanOperand],
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn compare_series(
        &self,
        _: yss_relational_contract::ComparisonOperation,
        _: &[yss_relational_contract::ComparisonOperand],
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidInput)
    }

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
    fn select_series(&self, _: &str) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidPlan)
    }
    fn project_series(&self, _: &[SeriesHandle]) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidPlan)
    }
    fn numeric_series(
        &self,
        _: NumericOperation,
        _: &[SeriesOperand],
        _: NumericType,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidPlan)
    }
    fn convert_series(
        &self,
        _: &SeriesHandle,
        _: yss_data_contract::SemanticConversion,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidPlan)
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
        let candidate = crate::session::build_current_project_candidate(
            ApplicationSessionEpoch::INITIAL,
            project,
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
                        semantic: None,
                        physical_type: None,
                        name: "x".into(),
                        data_type: yss_data_contract::ValueType::Scalar(
                            yss_data_contract::SemanticType::Numeric,
                        ),
                    }],
                },
            )]),
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
        let resource = PlanResourceId::from_existing("data".into());
        let version = PlanResourceVersion::from_existing("7".into());
        let basis = PlanBasis::new(
            session.clone(),
            PlanRegistryFingerprint::from_bytes([0; 32]),
            yss_node_kernel::KernelRegistry::default().fingerprint(),
            BTreeMap::from([(resource.clone(), version.clone())]),
            BTreeMap::from([(
                resource.clone(),
                PlanResourceObservedState::Present(version.clone()),
            )]),
        );
        let package = runtime
            .prepare_graph_package(&graph, &analysis, basis)
            .unwrap();
        let plan = runtime
            .prepare_package(package, runtime.generation())
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
                None,
                |_| {},
            )
            .unwrap();
        assert!(runtime.publish_committed_results(execution.handoff()));
        let result_id = execution.handoff().results()[0].result_id();
        let reference = yss_graph_execution::result::ResultReference {
            execution_session_id: captured.execution_session_id(),
            result_id,
        };
        let worker = std::thread::spawn(move || app.query_result_page(reference, 0, 1));
        entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        runtime.invalidate_graph_results(graph.as_str());
        resume_tx.send(()).unwrap();
        assert!(worker.join().unwrap().unwrap().is_none());
    }
}
