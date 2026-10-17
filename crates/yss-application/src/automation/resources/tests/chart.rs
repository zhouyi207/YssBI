use super::*;
use yss_assistant_contract::model::{ChartResourceRef, ChartSettingsUpdate};

fn read(f: &mut Fixture, resource: &ProjectResourceRef) -> ChartInspection {
    let AutomationCapabilityResult::ChartInspection(value) = f
        .call(AutomationCapabilityRequest::InspectChart(
            InspectChartRequest {
                chart: ChartResourceRef::new(resource.id.clone()),
                version: None,
            },
        ))
        .unwrap()
    else {
        panic!("chart inspection")
    };
    value
}

#[test]
fn chart_updates_preserve_omitted_settings_clear_axes_and_persist_at_the_read_baseline() {
    let mut f = Fixture::new();
    let resource = f.create(ResourceCreation::Chart {
        name: "Plot".into(),
    });
    let before = read(&mut f, &resource);
    let settings: ChartSettingsUpdate = serde_json::from_value(serde_json::json!({
        "databaseId":"sales", "chartType":"line", "x":"time", "y":"amount"
    }))
    .unwrap();
    let receipt = f.edit(&resource, ResourceEdit::UpdateChart { settings });
    assert_eq!(receipt.resources[0].dirty, Some(false));
    let after = read(&mut f, &resource);
    assert_eq!(
        after.settings,
        ChartSettings {
            database_id: "sales".into(),
            chart_type: ChartType::Line,
            x: Some("time".into()),
            y: Some("amount".into())
        }
    );

    let published = f.publications.len();
    let stale = f
        .call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: before.version.clone(),
                edit: ResourceEdit::UpdateChart {
                    settings: ChartSettingsUpdate {
                        x: Some(None),
                        ..Default::default()
                    },
                },
            },
        ))
        .unwrap_err();
    assert_eq!(stale.code, CapabilityFailureCode::REVISION_CONFLICT);
    assert_eq!(f.publications.len(), published);
    assert_eq!(read(&mut f, &resource), after);

    f.edit(
        &resource,
        ResourceEdit::UpdateChart {
            settings: serde_json::from_value(serde_json::json!({"y": null})).unwrap(),
        },
    );
    let cleared = read(&mut f, &resource);
    assert_eq!(cleared.settings.x.as_deref(), Some("time"));
    assert_eq!(cleared.settings.y, None);
    assert_eq!(cleared.settings.database_id, "sales");
    assert_eq!(cleared.settings.chart_type, ChartType::Line);
    let published = f.publications.len();
    assert!(
        f.call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: cleared.version.clone(),
                edit: ResourceEdit::UpdateChart {
                    settings: ChartSettingsUpdate::default()
                },
            }
        ))
        .is_err()
    );
    assert_eq!(f.publications.len(), published);
    let session = f.application.as_ref().unwrap().capture_session().unwrap();
    let rejected = f.application.as_ref().unwrap().apply_file_command(
        session.project_instance_id().clone(),
        OperationId::new(),
        FileCommand::Edit {
            path: file_path::<ChartDocument>(&resource).unwrap(),
            version: file_version(&before.version).unwrap(),
            edits: vec![yss_project_model::chart::ChartEdit::SetDocument {
                document: ChartDocument::new("stale"),
            }],
        },
    );
    assert!(matches!(
        rejected,
        Err(crate::file_resources::FileApplicationError::Project(
            yss_project::ProjectOperationError::ResourceRevisionConflict { .. }
        ))
    ));
    let disk: yss_chart_document::ChartDocument = serde_json::from_str(
        &std::fs::read_to_string(f.directory.join("project").join(&resource.id)).unwrap(),
    )
    .unwrap();
    assert_eq!(disk.chart_type, ChartType::Line);
    assert_eq!(disk.encodings.x.as_deref(), Some("time"));
    assert_eq!(disk.encodings.y, None);
}
