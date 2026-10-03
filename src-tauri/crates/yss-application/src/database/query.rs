use std::sync::Arc;

use yss_data_contract::TabularSnapshot;
use yss_database_contract::{DatabaseId, EditState};
use yss_database_runtime::error::DatabaseError;
use yss_database_runtime::runtime::DatabaseRuntimeSession;
use yss_database_runtime::{MAX_GET_DATAFRAME_ROWS, session_api};
use yss_database_schema::DatabaseColumnFact;
use yss_project_identity::{ProjectInstanceId, ResourceRevision};

use super::error::map_database_runtime_error;
use super::mutation::check_database_revision;
use super::{
    DatabaseApplicationOperation, DatabaseOperationError, DatabaseUseCaseError, database_id,
};
use crate::session::{ApplicationSession, ApplicationState};

#[derive(Debug)]
pub struct DatabaseMetaResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<DatabaseColumnFact>,
}

#[derive(Debug)]
pub struct DatabaseRowsResult {
    pub rows: TabularSnapshot,
    pub row_ids: Vec<i64>,
}

impl ApplicationState {
    pub fn query_database_meta_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        expected_revision: ResourceRevision,
    ) -> Result<DatabaseMetaResult, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        query_database_metadata_in_captured_session(self, &captured, &id, expected_revision)
    }

    pub fn query_database_rows_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        expected_revision: ResourceRevision,
        offset: usize,
        limit: usize,
    ) -> Result<DatabaseRowsResult, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        if limit > MAX_GET_DATAFRAME_ROWS {
            return Err(DatabaseUseCaseError::Database(
                DatabaseOperationError::RowLimitExceeded {
                    database_id: id,
                    operation: DatabaseApplicationOperation::ReadRows,
                    requested_rows: limit,
                    max_rows: MAX_GET_DATAFRAME_ROWS,
                },
            ));
        }
        read_database_in_captured_session(
            self,
            &captured,
            &id,
            expected_revision,
            DatabaseApplicationOperation::ReadRows,
            |session, database| {
                let page = session_api::page_snapshot(session, database, offset, limit)?;
                let (rows, row_ids) = page.into_parts();
                Ok(DatabaseRowsResult { rows, row_ids })
            },
        )
    }

    pub fn query_column_distributions_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        expected_revision: ResourceRevision,
    ) -> Result<Vec<yss_dataset_profile::ColumnDistribution>, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        read_database_in_captured_session(
            self,
            &captured,
            &id,
            expected_revision,
            DatabaseApplicationOperation::ColumnDistribution,
            session_api::column_distributions,
        )
    }

    pub fn query_database_edit_state_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        expected_revision: ResourceRevision,
    ) -> Result<EditState, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        read_database_in_captured_session(
            self,
            &captured,
            &id,
            expected_revision,
            DatabaseApplicationOperation::ReadEditState,
            session_api::edit_state,
        )
    }

    pub fn query_column_values_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        expected_revision: ResourceRevision,
        column: String,
    ) -> Result<Vec<String>, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        read_database_in_captured_session(
            self,
            &captured,
            &id,
            expected_revision,
            DatabaseApplicationOperation::ReadColumnValues,
            |session, database| session_api::column_values(session, database, &column),
        )
    }
}

pub(super) fn query_database_metadata_in_captured_session(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    id: &str,
    expected_revision: ResourceRevision,
) -> Result<DatabaseMetaResult, DatabaseUseCaseError> {
    read_database_in_captured_session(
        state,
        captured,
        id,
        expected_revision,
        DatabaseApplicationOperation::ReadMetadata,
        |session, database| {
            let snapshot = session_api::metadata_snapshot(session, database)?;
            Ok(DatabaseMetaResult {
                id: id.to_owned(),
                name: snapshot.name().to_owned(),
                row_count: snapshot.row_count(),
                column_count: snapshot.schema().columns().len(),
                columns: snapshot.schema().columns().to_vec(),
            })
        },
    )
}

fn read_database_in_captured_session<T>(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    id: &str,
    expected_revision: ResourceRevision,
    operation: DatabaseApplicationOperation,
    read: impl FnOnce(&DatabaseRuntimeSession, DatabaseId) -> Result<T, DatabaseError>,
) -> Result<T, DatabaseUseCaseError> {
    state
        .revalidate_captured_session(captured)
        .map_err(DatabaseUseCaseError::SessionChanged)?;
    check_database_revision(captured, id, expected_revision, operation)?;
    let database = database_id(id);
    let basis = captured
        .database()
        .capture_query_basis(&database)
        .map_err(|error| map_database_runtime_error(error, operation, id))?;
    if basis.declaration_revision().get() != expected_revision.get() {
        return Err(DatabaseUseCaseError::Database(
            DatabaseOperationError::StaleRevision {
                database_id: id.to_owned(),
                expected_revision,
            },
        ));
    }
    let result = read(captured.database(), database)
        .map_err(|error| map_database_runtime_error(error, operation, id))?;
    session_api::revalidate_query_basis(captured.database(), &basis)
        .map_err(|error| map_database_runtime_error(error, operation, id))?;
    check_database_revision(captured, id, expected_revision, operation)?;
    state
        .revalidate_captured_session(captured)
        .map_err(DatabaseUseCaseError::SessionChanged)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_database_contract::DatabaseImportSource;
    use yss_project_identity::OperationId;

    fn imported_database(
        contents: &str,
    ) -> (yss_project::fixtures::TempProject, ApplicationState, String) {
        let fixture = yss_project::fixtures::TempProject::activate(
            "database-query-session",
            yss_project_model::ProjectData::new(),
        );
        let root = fixture.state().capture_project_session().unwrap().root;
        let state = ApplicationState::initialize().unwrap();
        state
            .load_project_for_application(&root.as_path().to_string_lossy())
            .unwrap();
        let source = root.as_path().join("query.csv");
        std::fs::write(&source, contents).unwrap();
        let id = state
            .load_database_for_application(
                state
                    .capture_session()
                    .unwrap()
                    .project_instance_id()
                    .clone(),
                OperationId::new(),
                DatabaseImportSource::Csv {
                    path: source.to_string_lossy().into(),
                    delimiter: ',',
                    has_header: true,
                    infer_schema_length: Some(10),
                },
            )
            .unwrap()
            .data
            .id;
        (fixture, state, id)
    }

    fn revision(captured: &ApplicationSession, id: &str) -> ResourceRevision {
        captured
            .project()
            .read_project_index(captured.project_instance_id())
            .unwrap()
            .databases
            .into_iter()
            .find(|entry| entry.id == id)
            .unwrap()
            .revision
    }

    fn advance_project_declaration(captured: &ApplicationSession, id: &str) -> ResourceRevision {
        let project = captured.project();
        let instance = captured.project_instance_id();
        let before = revision(captured, id);
        let token = project
            .prepare_database_mutation_authority(instance, id, before)
            .unwrap();
        let mut declaration = project.read_database_declaration(instance, id).unwrap();
        declaration.name = "Project advanced".into();
        project
            .commit_database_declaration_update(instance, token, declaration, OperationId::new())
            .unwrap();
        before.checked_next().unwrap()
    }

    #[track_caller]
    fn assert_stale<T>(result: Result<T, DatabaseUseCaseError>, expected: ResourceRevision) {
        assert!(matches!(result, Err(DatabaseUseCaseError::Database(
            DatabaseOperationError::StaleRevision { expected_revision, .. }
        )) if expected_revision == expected));
    }

    #[test]
    fn reads_cannot_publish_or_restart_after_the_captured_session_is_replaced() {
        let (_fixture, state, id) = imported_database("value,other\n7,11\n");
        let captured = state.capture_session().unwrap();
        let expected = revision(&captured, &id);
        let result = read_database_in_captured_session(
            &state,
            &captured,
            &id,
            expected,
            DatabaseApplicationOperation::ReadMetadata,
            |session, database| {
                let result = session_api::metadata_snapshot(session, database)?;
                state.rebuild_application_session(&captured).unwrap();
                Ok(result)
            },
        );
        assert!(matches!(
            result,
            Err(DatabaseUseCaseError::SessionChanged(_))
        ));
        assert!(matches!(
            query_database_metadata_in_captured_session(&state, &captured, &id, expected),
            Err(DatabaseUseCaseError::SessionChanged(_))
        ));
        assert_eq!(
            state
                .query_database_meta_for_application(
                    captured.project_instance_id().clone(),
                    id,
                    expected
                )
                .unwrap()
                .row_count,
            1
        );
    }

    #[test]
    fn reads_require_the_requested_project_and_runtime_revision() {
        let (fixture, state, id) = imported_database("value,other\n7,11\n");
        let captured = state.capture_session().unwrap();
        let instance = captured.project_instance_id().clone();
        let old_revision = revision(&captured, &id);
        state
            .mutate_database_for_application(
                instance.clone(),
                id.clone(),
                old_revision,
                OperationId::new(),
                crate::database::DatabaseMutation::AddRow { index: None },
            )
            .unwrap();
        let current_revision = revision(&captured, &id);
        let export = fixture
            .state()
            .capture_project_session()
            .unwrap()
            .root
            .as_path()
            .join("existing.csv");
        std::fs::write(&export, "retained export").unwrap();
        assert_eq!(
            state
                .query_database_rows_for_application(
                    instance.clone(),
                    id.clone(),
                    current_revision,
                    0,
                    10,
                )
                .unwrap()
                .rows
                .row_count(),
            2
        );

        let assert_queries_stale = |expected| {
            assert_stale(
                state.query_column_values_for_application(
                    instance.clone(),
                    id.clone(),
                    expected,
                    "value".into(),
                ),
                expected,
            );
            assert_stale(
                state.query_database_meta_for_application(instance.clone(), id.clone(), expected),
                expected,
            );
            assert_stale(
                state.query_database_rows_for_application(
                    instance.clone(),
                    id.clone(),
                    expected,
                    0,
                    10,
                ),
                expected,
            );
            assert_stale(
                state.query_column_distributions_for_application(
                    instance.clone(),
                    id.clone(),
                    expected,
                ),
                expected,
            );
            assert_stale(
                state.query_database_edit_state_for_application(
                    instance.clone(),
                    id.clone(),
                    expected,
                ),
                expected,
            );
            assert_stale(
                state.export_database_for_application(
                    instance.clone(),
                    id.clone(),
                    export.to_string_lossy().into(),
                    "csv".into(),
                    Some(expected),
                ),
                expected,
            );
            assert_eq!(std::fs::read_to_string(&export).unwrap(), "retained export");
        };
        let plot = |expected, y_column: &str| {
            state.query_chart_plot(crate::chart::ChartPlotQuery {
                project_instance_id: instance.clone(),
                database_id: database_id(&id),
                expected_revision: expected,
                x_column: "value".try_into().unwrap(),
                y_column: y_column.try_into().unwrap(),
                max_points: Some(10),
            })
        };
        assert_eq!(plot(current_revision, "other").unwrap().data.len(), 1);
        assert_eq!(
            plot(current_revision, "value").unwrap().data,
            vec![crate::chart::PlotPoint { x: 7.0, y: 7.0 }]
        );
        assert_queries_stale(old_revision);
        assert!(matches!(
            plot(old_revision, "other"),
            Err(crate::chart::ChartPlotApplicationError::ProjectAuthorityChanged { .. })
        ));
        let project_revision = advance_project_declaration(&captured, &id);
        assert_queries_stale(project_revision);
        assert!(matches!(plot(project_revision, "other"),
            Err(crate::chart::ChartPlotApplicationError::Database(error))
            if error.kind() == yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::RuntimeRevisionMismatch));
    }

    #[test]
    fn complete_exact_column_values_initialize_all_domain_semantics_without_changing_rows() {
        use yss_data_contract::{ColumnSemantic, SemanticType, SemanticValue};
        let contents = format!(
            "value,label\n{}9007199254740995,late\n,missing\n",
            "9007199254740993,same\n".repeat(205)
        );
        let (_fixture, state, id) = imported_database(&contents);
        let captured = state.capture_session().unwrap();
        let instance = captured.project_instance_id().clone();
        let initial_revision = revision(&captured, &id);
        let values = state
            .query_column_values_for_application(
                instance.clone(),
                id.clone(),
                initial_revision,
                "value".into(),
            )
            .unwrap();
        assert_eq!(values, ["9007199254740993", "9007199254740995"]);
        assert_eq!(
            state
                .query_column_values_for_application(
                    instance.clone(),
                    id.clone(),
                    initial_revision,
                    "label".into()
                )
                .unwrap(),
            ["late", "missing", "same"]
        );
        assert_eq!(revision(&captured, &id), initial_revision);
        assert!(
            !state
                .query_database_edit_state_for_application(
                    instance.clone(),
                    id.clone(),
                    initial_revision
                )
                .unwrap()
                .can_undo
        );
        assert!(
            state
                .query_column_values_for_application(
                    instance.clone(),
                    id.clone(),
                    initial_revision,
                    "missing_column".into()
                )
                .is_err()
        );
        for kind in [
            SemanticType::Categorical,
            SemanticType::Ordinal,
            SemanticType::Binary,
        ] {
            let semantic = ColumnSemantic {
                kind,
                values: values
                    .iter()
                    .map(|value| SemanticValue {
                        value: value.clone(),
                        label: value.clone(),
                    })
                    .collect(),
                positive_value: None,
                numeric: None,
            };
            state
                .mutate_database_for_application(
                    instance.clone(),
                    id.clone(),
                    revision(&captured, &id),
                    OperationId::new(),
                    crate::database::DatabaseMutation::SetColumnSemantic {
                        column: "value".into(),
                        semantic: semantic.clone(),
                    },
                )
                .unwrap();
            let current = revision(&captured, &id);
            let metadata = state
                .query_database_meta_for_application(instance.clone(), id.clone(), current)
                .unwrap();
            assert_eq!(metadata.row_count, 207);
            assert_eq!(metadata.columns[0].semantic(), Some(&semantic));
            assert_eq!(
                state
                    .query_column_values_for_application(
                        instance.clone(),
                        id.clone(),
                        current,
                        "value".into()
                    )
                    .unwrap(),
                values
            );
        }
        assert_stale(
            state.query_column_values_for_application(
                instance,
                id,
                initial_revision,
                "value".into(),
            ),
            initial_revision,
        );
    }

    #[test]
    fn read_cannot_publish_after_project_revision_changes_during_conversion() {
        let (_fixture, state, id) = imported_database("value,other\n7,11\n");
        let captured = state.capture_session().unwrap();
        let expected = revision(&captured, &id);
        let result = read_database_in_captured_session(
            &state,
            &captured,
            &id,
            expected,
            DatabaseApplicationOperation::ReadMetadata,
            |session, database| {
                let result = session_api::metadata_snapshot(session, database)?;
                advance_project_declaration(&captured, &id);
                Ok(result)
            },
        );
        assert_stale(result, expected);
    }
}
