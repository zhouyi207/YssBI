use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use yss_data_contract::{TabularColumn, TabularColumnName, TabularScalar, TabularSnapshot};
use yss_graph_document::{GraphResourcePath, NodeId, PortAddress};
use yss_graph_execution::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};
use yss_node_kernel::RuntimeValue;
use yss_relational_contract::{
    NumericOperation, NumericType, RelationBatchStream, RelationBinding, RelationColumn,
    RelationControl, RelationError, RelationExecutor, RelationFuture, RelationHandle, RelationPage,
    RelationPlan, RelationPredicate, SeriesHandle, SeriesOperand, SeriesPlan,
};

use crate::session::{ApplicationSessionEpoch, ApplicationSessionSlot, ApplicationState};

struct DelayedPage {
    binding: RelationBinding,
    entered: mpsc::SyncSender<()>,
    resume: Mutex<mpsc::Receiver<()>>,
    fail: bool,
}

impl RelationPlan for DelayedPage {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn concat_rows(
        &self,
        _: &[RelationHandle],
        _: yss_data_contract::table::RowConcatMode,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn concat_columns(&self, _: &[RelationHandle]) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn join(
        &self,
        _: &RelationHandle,
        _: &yss_data_contract::table::TableJoin,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }

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

    fn bindings(&self) -> &[RelationBinding] {
        std::slice::from_ref(&self.binding)
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
    fn drop_rows(&self, _: &RelationPredicate) -> Result<RelationHandle, RelationError> {
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
    fn project_series(
        &self,
        _: &[SeriesHandle],
        _: Option<&[Box<str>]>,
    ) -> Result<RelationHandle, RelationError> {
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
        let session = captured.project_session_id();
        let runtime = captured.execution();
        let graph = GraphResourcePath::new("events/paged.yssbi-event").unwrap();
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
        let result_id = runtime.publish_fixture_result(
            PlanOutputRef::new(
                PlanGraphId::from_existing(graph.as_str().into()),
                PlanPortAddress::from_existing(
                    PortAddress::declared(NodeId::new(), "dataframe".parse().unwrap())
                        .to_string()
                        .into(),
                ),
            ),
            yss_graph_execution::result::StoredResult::new(RuntimeValue::Relation(relation)),
        );
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
