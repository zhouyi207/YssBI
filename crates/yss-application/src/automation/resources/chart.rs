//! Chart settings reuse the original reader and immediate-save transaction.
use super::*;

pub(in crate::automation) fn inspect_chart(
    application: &ApplicationState,
    session: &ApplicationSession,
    request: InspectChartRequest,
    control: &CapabilityControl,
) -> Result<ChartInspection> {
    let resource = request.chart.resource();
    let index = read_index(session)?;
    let info = metadata(&index, &resource)?;
    let version = ResourceVersion {
        revision: info.revision,
        session_id: None,
    };
    if request
        .version
        .as_ref()
        .is_some_and(|expected| expected != &version)
    {
        return Err(conflict());
    }
    let document = application
        .load_chart_resource(
            session.project_instance_id().clone(),
            chart_path(&resource)?,
            Some(index.publication_revision),
        )
        .map_err(chart_error)?;
    check_version(session, &resource, &version)?;
    control.check()?;
    Ok(ChartInspection {
        chart: request.chart,
        version,
        settings: ChartSettings {
            database_id: document.database_id,
            chart_type: document.chart_type,
            x: document.encodings.x,
            y: document.encodings.y,
        },
    })
}

pub(super) fn update_chart(
    application: &ApplicationState,
    session: &ApplicationSession,
    resource: &ProjectResourceRef,
    version: &ResourceVersion,
    settings: model::ChartSettingsUpdate,
    operation: OperationId,
    control: &CapabilityControl,
) -> Result<CommittedResourceMutation> {
    let project = session.project_instance_id().clone();
    let path = chart_path(resource)?;
    let mut document = application
        .load_chart_resource(project.clone(), path.clone(), None)
        .map_err(chart_error)?;
    check_version(session, resource, version)?;
    if let Some(database_id) = settings.database_id {
        document.database_id = database_id;
    }
    if let Some(chart_type) = settings.chart_type {
        document.chart_type = chart_type;
    }
    if let Some(x) = settings.x {
        document.encodings.x = x;
    }
    if let Some(y) = settings.y {
        document.encodings.y = y;
    }
    control.check()?;
    application
        .save_chart_resource(
            project,
            operation,
            path,
            document,
            Some(ResourceRevision::new(version.revision)),
        )
        .map_err(chart_error)
}
