//! Chart settings reuse the shared file snapshot and Edit/Save lifecycle.
use super::*;
use yss_project_model::chart::ChartEdit;

pub(in crate::automation) fn inspect_chart(
    application: &ApplicationState,
    session: &ApplicationSession,
    request: InspectChartRequest,
    control: &CapabilityControl,
) -> Result<ChartInspection> {
    let resource = request.chart.resource();
    let chart = application
        .read_file(
            session.project_instance_id().clone(),
            file_path::<ChartDocument>(&resource)?,
        )
        .map_err(file_error)?;
    let version = ResourceVersion {
        revision: chart.version.revision.get(),
        session_id: Some(chart.version.session_id.clone()),
    };
    if request
        .version
        .as_ref()
        .is_some_and(|expected| !matches_file_version(expected, &chart.version))
    {
        return Err(conflict());
    }
    control.check()?;
    Ok(ChartInspection {
        chart: request.chart,
        version,
        settings: ChartSettings {
            database_id: chart.content.database_id,
            chart_type: chart.content.chart_type,
            x: chart.content.encodings.x,
            y: chart.content.encodings.y,
        },
    })
}

pub(super) fn update_chart(
    application: &ApplicationState,
    session: &ApplicationSession,
    resource: &ProjectResourceRef,
    version: &ResourceVersion,
    settings: model::ChartSettingsUpdate,
    control: &CapabilityControl,
    publish: Publication<'_>,
) -> Result<CommittedResourceMutation> {
    let project = session.project_instance_id().clone();
    let path = file_path::<ChartDocument>(resource)?;
    let current = application
        .read_file(project.clone(), path.clone())
        .map_err(file_error)?;
    if !matches_file_version(version, &current.version) {
        return Err(conflict());
    }
    let mut document = current.content;
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
    // Settings updates persist immediately, so the edit is followed by the same Save as the GUI.
    let edited = application
        .apply_file_command(
            project.clone(),
            OperationId::new(),
            FileCommand::Edit {
                path: path.clone(),
                version: current.version,
                edits: vec![ChartEdit::SetDocument { document }],
            },
        )
        .map_err(file_error)?;
    publish_committed(publish, &edited.mutation);
    let version = edited.snapshot.ok_or_else(unavailable)?.version;
    file_command(
        application,
        project,
        OperationId::new(),
        FileCommand::Save { path, version },
    )
}

fn matches_file_version(expected: &ResourceVersion, current: &FileVersion) -> bool {
    expected.revision == current.revision.get()
        && expected
            .session_id
            .as_ref()
            .is_none_or(|session_id| session_id == &current.session_id)
}
