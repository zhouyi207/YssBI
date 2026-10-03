use yss_data_contract::TabularColumnName;
use yss_database_contract::{DatabaseDeclarationRevision, DatabaseId};
use yss_database_runtime::plot_query::{
    self, DatabasePlotQueryError, NumericColumnKind, NumericColumnPair,
};
use yss_project_identity::{ProjectInstanceId, ResourceRevision};

use crate::session::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};

use super::projection::{ChartPlotInput, ChartPlotResult, PlotAxisFormat, project_chart_plot};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChartPlotQuery {
    pub project_instance_id: ProjectInstanceId,
    pub database_id: DatabaseId,
    pub expected_revision: ResourceRevision,
    pub x_column: TabularColumnName,
    pub y_column: TabularColumnName,
    pub max_points: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum ChartPlotApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("captured chart plot session changed")]
    SessionChanged,
    #[error("chart plot project identity changed")]
    ProjectIdentityMismatch { requested: ProjectInstanceId },
    #[error("chart plot project authority changed")]
    ProjectAuthorityChanged { database: DatabaseId },
    #[error(transparent)]
    Database(#[from] DatabasePlotQueryError),
    #[error("chart plot has no finite points")]
    PlotDataEmpty,
}

impl<'a> From<&'a NumericColumnPair> for ChartPlotInput<'a> {
    fn from(pair: &'a NumericColumnPair) -> Self {
        Self {
            x: pair.x(),
            y: pair.y(),
            x_label: pair.x_label(),
            y_label: pair.y_label(),
            x_format: axis_format(pair.x_kind()),
            y_format: axis_format(pair.y_kind()),
        }
    }
}

impl ApplicationState {
    pub fn query_chart_plot(
        &self,
        query: ChartPlotQuery,
    ) -> Result<ChartPlotResult, ChartPlotApplicationError> {
        let captured = self.capture_session()?;
        let result = query_chart_plot_in_session(&captured, query)?;
        self.revalidate_captured_session(&captured)
            .map_err(map_session_revalidation_error)?;
        Ok(result)
    }
}

fn query_chart_plot_in_session(
    session: &ApplicationSession,
    query: ChartPlotQuery,
) -> Result<ChartPlotResult, ChartPlotApplicationError> {
    if query.project_instance_id != *session.project_instance_id() {
        return Err(ChartPlotApplicationError::ProjectIdentityMismatch {
            requested: query.project_instance_id,
        });
    }

    check_project_database_revision(session, &query)?;
    let pair = plot_query::read_numeric_column_pair(
        session.database(),
        &query.database_id,
        DatabaseDeclarationRevision::from_existing(query.expected_revision.get()),
        &query.x_column,
        &query.y_column,
    )?;
    let result = project_chart_plot(ChartPlotInput::from(&pair), query.max_points)
        .ok_or(ChartPlotApplicationError::PlotDataEmpty)?;
    check_project_database_revision(session, &query)?;
    plot_query::revalidate_numeric_column_pair(session.database(), &pair)?;
    Ok(result)
}

fn check_project_database_revision(
    session: &ApplicationSession,
    query: &ChartPlotQuery,
) -> Result<(), ChartPlotApplicationError> {
    session
        .project()
        .prepare_database_mutation_authority(
            session.project_instance_id(),
            query.database_id.as_str(),
            query.expected_revision,
        )
        .map(|_| ())
        .map_err(|_| ChartPlotApplicationError::ProjectAuthorityChanged {
            database: query.database_id.clone(),
        })
}

fn map_session_revalidation_error(error: SessionRevalidationError) -> ChartPlotApplicationError {
    match error {
        SessionRevalidationError::Unavailable(error) => {
            ChartPlotApplicationError::SessionCapture(error)
        }
        SessionRevalidationError::Changed => ChartPlotApplicationError::SessionChanged,
    }
}

fn axis_format(kind: NumericColumnKind) -> PlotAxisFormat {
    match kind {
        NumericColumnKind::Number => PlotAxisFormat::Number,
        NumericColumnKind::Date => PlotAxisFormat::Date,
        NumericColumnKind::Datetime => PlotAxisFormat::Datetime,
    }
}
