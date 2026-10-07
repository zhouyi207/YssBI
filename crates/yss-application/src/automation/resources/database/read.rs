//! Database tool reads use the Application query gate and the original runtime snapshot.
use super::*;
use model::DatabaseReadInput as Input;
use yss_relational_contract::{RelationComparison, RelationControl, RelationPredicate, SortColumn};

mod profile;

pub(in crate::automation) fn read_database(
    application: &ApplicationState,
    session: &ApplicationSession,
    request: DatabaseReadRequest,
    control: &CapabilityControl,
) -> Result<DatabaseReadResult> {
    let database = request.input.database().clone();
    let basis = session
        .database()
        .capture_query_basis(&DatabaseId::from_existing(
            database.id.clone().into_boxed_str(),
        ))
        .map_err(|error| crate::automation::map_dataset_profile_error(&error, &database.id))?;
    let version = ResourceVersion {
        revision: basis.declaration_revision().get(),
        session_id: None,
    };
    if request
        .version
        .as_ref()
        .is_some_and(|expected| expected != &version)
    {
        return Err(conflict());
    }
    let revision = ResourceRevision::new(version.revision);
    let project = session.project_instance_id().clone();
    let metadata = application
        .query_database_meta_for_application(project.clone(), database.id.clone(), revision)
        .map_err(|error| read_error(error, &database.id))?;
    let relation_control = RelationControl {
        cancellation: control.cancellation_flag(),
        deadline: control.deadline(),
        max_input_bytes: 16 * 1024 * 1024,
    };
    let content = match request.input {
        Input::Overview(_) => {
            let state = application
                .query_database_edit_state_for_application(project, database.id.clone(), revision)
                .map_err(|error| read_error(error, &database.id))?;
            DatabaseReadContent::Overview {
                name: metadata.name,
                row_count: metadata.row_count,
                column_count: metadata.column_count,
                dirty: state.is_modified,
                can_undo: state.can_undo,
                can_redo: state.can_redo,
            }
        }
        Input::Schema(input) => {
            let columns = if input.columns.is_empty() {
                metadata.columns.iter().collect::<Vec<_>>()
            } else {
                input
                    .columns
                    .iter()
                    .map(|name| column(&metadata.columns, name))
                    .collect::<Result<Vec<_>>>()?
            };
            let total = columns.len();
            let columns = columns
                .into_iter()
                .skip(input.offset)
                .take(input.limit)
                .map(schema_column)
                .collect::<Vec<_>>();
            let page = InspectionPage::known(input.offset, columns.len(), total);
            DatabaseReadContent::Schema { columns, page }
        }
        Input::Rows(input) => {
            for name in input
                .columns
                .iter()
                .chain(input.filters.iter().map(|value| &value.column))
                .chain(input.order.iter().map(|value| &value.column))
            {
                column(&metadata.columns, name)?;
            }
            let query = yss_database_runtime::DatasetRowsQuery {
                columns: input.columns,
                filters: input
                    .filters
                    .into_iter()
                    .map(|value| RelationPredicate {
                        column: value.column.into(),
                        comparison: comparison(value.comparison),
                        value: value.value,
                    })
                    .collect(),
                order: input
                    .order
                    .into_iter()
                    .map(|value| SortColumn {
                        column: value.column.into(),
                        ascending: value.ascending,
                        nulls_first: value.nulls_first,
                    })
                    .collect(),
                offset: input.offset,
                limit: input.limit,
            };
            let page = application
                .query_database_rows_selected_for_application(
                    project,
                    database.id.clone(),
                    revision,
                    &query,
                    &relation_control,
                )
                .map_err(|error| read_error(error, &database.id))?;
            let rows = (0..page.rows.row_count())
                .map(|row| {
                    page.rows
                        .columns()
                        .iter()
                        .map(|column| column.values()[row].clone())
                        .collect()
                })
                .collect::<Vec<_>>();
            let end = input.offset.saturating_add(rows.len());
            DatabaseReadContent::Rows {
                columns: query.columns,
                row_ids: page.row_ids,
                page: InspectionPage {
                    offset: input.offset,
                    returned: rows.len(),
                    total: if query.filters.is_empty() {
                        Some(metadata.row_count)
                    } else {
                        None
                    },
                    has_more: page.has_more,
                    next_offset: page.has_more.then_some(end),
                },
                rows,
            }
        }
        Input::Profile(input) => {
            for name in &input.columns {
                column(&metadata.columns, name)?;
            }
            let query = session_api::DatabaseProfileQuery {
                columns: input.columns,
                metrics: input
                    .metrics
                    .into_iter()
                    .map(|value| match value {
                        model::DatabaseProfileMetric::Completeness => {
                            session_api::DatabaseProfileMetric::Completeness
                        }
                        model::DatabaseProfileMetric::Statistics => {
                            session_api::DatabaseProfileMetric::Statistics
                        }
                        model::DatabaseProfileMetric::Distribution => {
                            session_api::DatabaseProfileMetric::Distribution
                        }
                    })
                    .collect(),
            };
            let snapshot = application
                .query_database_profile_for_application(
                    project,
                    database.id.clone(),
                    revision,
                    &query,
                    &relation_control,
                )
                .map_err(|error| read_error(error, &database.id))?;
            DatabaseReadContent::Profile {
                columns: query.columns,
                metrics: profile::project(snapshot),
            }
        }
    };
    session_api::revalidate_query_basis(session.database(), &basis).map_err(|_| conflict())?;
    control.check()?;
    Ok(DatabaseReadResult {
        database,
        version,
        content,
    })
}

fn column<'a>(
    columns: &'a [yss_database_schema::DatabaseColumnFact],
    name: &str,
) -> Result<&'a yss_database_schema::DatabaseColumnFact> {
    columns
        .iter()
        .find(|column| column.name().as_str() == name)
        .ok_or_else(|| {
            invalid("columns").with_detail("column", name).with_detail(
                "nextStep",
                "Inspect the database schema and use an existing column name.",
            )
        })
}

fn schema_column(column: &yss_database_schema::DatabaseColumnFact) -> DatasetColumnSchema {
    DatasetColumnSchema {
        name: column.name().as_str().into(),
        data_type: column.data_type().to_string(),
        physical_type: column.physical_type().into(),
        semantic: column.semantic().map(semantic_to_contract),
        nullable: column.nullable(),
    }
}

fn comparison(value: model::DatabaseComparison) -> RelationComparison {
    use model::DatabaseComparison as C;
    match value {
        C::Equal => RelationComparison::Equal,
        C::NotEqual => RelationComparison::NotEqual,
        C::Less => RelationComparison::Less,
        C::LessEqual => RelationComparison::LessEqual,
        C::Greater => RelationComparison::Greater,
        C::GreaterEqual => RelationComparison::GreaterEqual,
        C::IsNull => RelationComparison::IsNull,
        C::IsNotNull => RelationComparison::IsNotNull,
    }
}

fn read_error(
    error: crate::database::DatabaseUseCaseError,
    database_id: &str,
) -> CapabilityFailure {
    use crate::database::{DatabaseOperationError as D, DatabaseUseCaseError as E};
    use std::error::Error;
    if let E::Database(D::Internal(internal)) = &error
        && let Some(source) = internal
            .source()
            .and_then(|source| source.downcast_ref::<yss_database_runtime::error::DatabaseError>())
    {
        return crate::automation::map_dataset_profile_error(source, database_id);
    }
    match error {
        E::Database(D::InvalidInput { field, .. }) => invalid(field),
        other => {
            let failure = database_error(other);
            if failure.code == CapabilityFailureCode::MutationRejected {
                CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
            } else {
                failure
            }
        }
    }
}
