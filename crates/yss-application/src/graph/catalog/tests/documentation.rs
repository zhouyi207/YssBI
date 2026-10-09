use super::*;

#[test]
fn node_documentation_matches_localized_catalog_without_requiring_execution() {
    let staged = staged_session(
        ProjectData::new(),
        "node-documentation",
        GraphRuntimeTestControl::default(),
    );
    let project = staged.session.project_instance_id();
    // Exercise embedded help and the generated help of an unavailable definition.
    for language in ["en-US", "zh-CN"] {
        let catalog = staged
            .session
            .graph()
            .localized_catalog_with_resources(&[], language);
        for id in ["yssbi.statistics.describe", "yssbi.dataframe.impute.mice"] {
            let node_type = NodeTypeId::new(id).unwrap();
            let document = staged
                .application
                .node_documentation(project, &node_type, language)
                .unwrap();
            assert!(document.as_ref().is_some_and(|text| !text.is_empty()));
            assert_eq!(
                document,
                catalog
                    .items
                    .iter()
                    .find(|item| item.node_type_id.as_ref() == id)
                    .unwrap()
                    .documentation
            );
        }
    }
    let describe = NodeTypeId::new("yssbi.statistics.describe").unwrap();
    let english = staged
        .application
        .node_documentation(project, &describe, "en-US")
        .unwrap();
    let chinese = staged
        .application
        .node_documentation(project, &describe, "ZH_tw")
        .unwrap();
    assert_ne!(english, chinese);
    assert_eq!(
        english,
        staged
            .application
            .node_documentation(project, &describe, "fr-FR")
            .unwrap()
    );
    assert_eq!(
        staged
            .application
            .node_documentation(
                project,
                &NodeTypeId::new("yssbi.test.missing").unwrap(),
                "en-US",
            )
            .unwrap(),
        None
    );
}

#[test]
fn node_documentation_rejects_foreign_projects_and_inactive_sessions() {
    let staged = staged_session(
        ProjectData::new(),
        "node-documentation-stale",
        GraphRuntimeTestControl::default(),
    );
    let node_type = NodeTypeId::new("yssbi.statistics.describe").unwrap();
    assert!(matches!(
        staged.application.node_documentation(
            &ProjectInstanceId::from_existing("another-project".into()),
            &node_type,
            "en-US",
        ),
        Err(CatalogQueryApplicationError::CatalogProjectStale)
    ));
    let inactive = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(
        crate::session::NodeComponents::builtins().unwrap(),
    )));
    assert!(matches!(
        inactive.node_documentation(staged.session.project_instance_id(), &node_type, "en-US"),
        Err(CatalogQueryApplicationError::SessionCapture(
            SessionCaptureError::Inactive
        ))
    ));
    assert!(staged.control.events().is_empty());
}
