use crate::database::plot_query::{
    self, DatabasePlotQueryError, NumericColumnKind, NumericColumnPair,
};
use yss_database_contract::DatabaseId;
use yss_project_identity::{ProjectInstanceId, ResourceRevision};
use yss_tabular_contract::TabularColumnName;

use super::execution::session_slot::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};

pub const DEFAULT_MAX_PLOT_POINTS: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlotAxisFormat {
    Number,
    Date,
    Datetime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorksheetPlotQuery {
    pub project_instance_id: ProjectInstanceId,
    pub database_id: DatabaseId,
    pub x_column: TabularColumnName,
    pub y_column: TabularColumnName,
    pub max_points: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorksheetPlotResult {
    pub data: Vec<PlotPoint>,
    pub x_label: Option<Box<str>>,
    pub y_label: Option<Box<str>>,
    pub x_format: PlotAxisFormat,
    pub y_format: PlotAxisFormat,
}

#[derive(Debug, thiserror::Error)]
pub enum WorksheetPlotApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("captured worksheet plot session changed")]
    SessionChanged,
    #[error("worksheet plot project identity changed")]
    ProjectIdentityMismatch { requested: ProjectInstanceId },
    #[error("worksheet plot project authority changed")]
    ProjectAuthorityChanged { database: DatabaseId },
    #[error(transparent)]
    Database(#[from] DatabasePlotQueryError),
    #[error("worksheet plot has no finite points")]
    PlotDataEmpty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectPlotAuthorityFacts {
    project_instance_id: ProjectInstanceId,
    publication_revision: u64,
    database_revision: ResourceRevision,
}

#[derive(Clone, Copy)]
struct NumericPairView<'a> {
    x: &'a [Option<f64>],
    y: &'a [Option<f64>],
    x_label: Option<&'a str>,
    y_label: Option<&'a str>,
    x_kind: NumericColumnKind,
    y_kind: NumericColumnKind,
}

impl<'a> From<&'a NumericColumnPair> for NumericPairView<'a> {
    fn from(pair: &'a NumericColumnPair) -> Self {
        Self {
            x: pair.x(),
            y: pair.y(),
            x_label: pair.x_label(),
            y_label: pair.y_label(),
            x_kind: pair.x_kind(),
            y_kind: pair.y_kind(),
        }
    }
}

impl ApplicationState {
    pub fn query_worksheet_plot(
        &self,
        query: WorksheetPlotQuery,
    ) -> Result<WorksheetPlotResult, WorksheetPlotApplicationError> {
        let captured = self.capture_session()?;
        let result = query_worksheet_plot_in_session(&captured, query)?;
        self.revalidate_captured_session(&captured)
            .map_err(map_session_revalidation_error)?;
        Ok(result)
    }
}

pub(crate) fn query_worksheet_plot_in_session(
    session: &ApplicationSession,
    query: WorksheetPlotQuery,
) -> Result<WorksheetPlotResult, WorksheetPlotApplicationError> {
    if query.project_instance_id != *session.project_instance_id() {
        return Err(WorksheetPlotApplicationError::ProjectIdentityMismatch {
            requested: query.project_instance_id,
        });
    }

    let project_facts = capture_project_authority_facts(session, &query.database_id)?;
    let pair = plot_query::read_numeric_column_pair(
        session.database(),
        &query.database_id,
        &query.x_column,
        &query.y_column,
    )?;
    let result = worksheet_plot_from_pair(NumericPairView::from(&pair), query.max_points)?;
    revalidate_project_authority_facts(session, &project_facts, &query.database_id)?;
    plot_query::revalidate_numeric_column_pair(session.database(), &pair)?;
    Ok(result)
}

fn capture_project_authority_facts(
    session: &ApplicationSession,
    database: &DatabaseId,
) -> Result<ProjectPlotAuthorityFacts, WorksheetPlotApplicationError> {
    let index = session
        .project()
        .read_project_index(session.project_instance_id())
        .map_err(|_| WorksheetPlotApplicationError::ProjectAuthorityChanged {
            database: database.clone(),
        })?;
    if index.project_instance_id != session.project_instance_id().as_str() {
        return Err(WorksheetPlotApplicationError::ProjectIdentityMismatch {
            requested: session.project_instance_id().clone(),
        });
    }
    let database_revision = index
        .databases
        .iter()
        .find(|entry| entry.id == database.as_str())
        .map(|entry| entry.revision)
        .ok_or_else(|| WorksheetPlotApplicationError::ProjectAuthorityChanged {
            database: database.clone(),
        })?;
    Ok(ProjectPlotAuthorityFacts {
        project_instance_id: session.project_instance_id().clone(),
        publication_revision: index.publication_revision,
        database_revision,
    })
}

fn revalidate_project_authority_facts(
    session: &ApplicationSession,
    expected: &ProjectPlotAuthorityFacts,
    database: &DatabaseId,
) -> Result<(), WorksheetPlotApplicationError> {
    let current = capture_project_authority_facts(session, database)?;
    if current.project_instance_id != expected.project_instance_id {
        return Err(WorksheetPlotApplicationError::ProjectIdentityMismatch {
            requested: expected.project_instance_id.clone(),
        });
    }
    if current.publication_revision != expected.publication_revision
        || current.database_revision != expected.database_revision
    {
        return Err(WorksheetPlotApplicationError::ProjectAuthorityChanged {
            database: database.clone(),
        });
    }
    Ok(())
}

fn map_session_revalidation_error(
    error: SessionRevalidationError,
) -> WorksheetPlotApplicationError {
    match error {
        SessionRevalidationError::Unavailable(error) => {
            WorksheetPlotApplicationError::SessionCapture(error)
        }
        SessionRevalidationError::Changed => WorksheetPlotApplicationError::SessionChanged,
    }
}

fn worksheet_plot_from_pair(
    pair: NumericPairView<'_>,
    max_points: Option<usize>,
) -> Result<WorksheetPlotResult, WorksheetPlotApplicationError> {
    let data = pair
        .x
        .iter()
        .zip(pair.y.iter())
        .filter_map(|(x, y)| match (x, y) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => {
                Some(PlotPoint { x: *x, y: *y })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if data.is_empty() {
        return Err(WorksheetPlotApplicationError::PlotDataEmpty);
    }
    let max_points = max_points.unwrap_or(DEFAULT_MAX_PLOT_POINTS);
    Ok(WorksheetPlotResult {
        data: subsample_points(data, max_points),
        x_label: pair.x_label.map(Into::into),
        y_label: pair.y_label.map(Into::into),
        x_format: axis_format(pair.x_kind),
        y_format: axis_format(pair.y_kind),
    })
}

fn subsample_points<T>(data: Vec<T>, max_points: usize) -> Vec<T> {
    if data.len() <= max_points {
        return data;
    }
    if max_points == 0 {
        return Vec::new();
    }
    let stride = data.len() / max_points + usize::from(data.len() % max_points != 0);
    data.into_iter().step_by(stride).take(max_points).collect()
}

fn axis_format(kind: NumericColumnKind) -> PlotAxisFormat {
    match kind {
        NumericColumnKind::Number => PlotAxisFormat::Number,
        NumericColumnKind::Date => PlotAxisFormat::Date,
        NumericColumnKind::Datetime => PlotAxisFormat::Datetime,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_worksheet_plot_filters_caps_and_preserves_formats_from_same_session() {
        let x = [Some(1.0), Some(f64::NAN), Some(3.0), Some(4.0), Some(5.0)];
        let y = [
            Some(10.0),
            Some(20.0),
            Some(f64::INFINITY),
            Some(40.0),
            Some(50.0),
        ];
        let result = worksheet_plot_from_pair(
            NumericPairView {
                x: &x,
                y: &y,
                x_label: Some("observed_date"),
                y_label: Some("measure"),
                x_kind: NumericColumnKind::Date,
                y_kind: NumericColumnKind::Number,
            },
            Some(2),
        )
        .expect("finite points remain after filtering");

        assert_eq!(
            result.data,
            vec![PlotPoint { x: 1.0, y: 10.0 }, PlotPoint { x: 5.0, y: 50.0 },]
        );
        assert_eq!(result.x_label.as_deref(), Some("observed_date"));
        assert_eq!(result.y_label.as_deref(), Some("measure"));
        assert_eq!(result.x_format, PlotAxisFormat::Date);
        assert_eq!(result.y_format, PlotAxisFormat::Number);
        assert!(
            worksheet_plot_from_pair(
                NumericPairView {
                    x: &x,
                    y: &y,
                    x_label: None,
                    y_label: None,
                    x_kind: NumericColumnKind::Datetime,
                    y_kind: NumericColumnKind::Number,
                },
                Some(0),
            )
            .expect("zero cap preserves the existing empty-success behavior")
            .data
            .is_empty()
        );
    }
}
