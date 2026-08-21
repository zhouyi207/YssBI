use super::*;

#[test]
fn localized_catalog_rejects_stale_project_identity() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-localized-catalog-stale-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let stale = state.capture_project_session().unwrap().instance_id;
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());

    let error = get_localized_node_catalog_from_state(&state, stale, "en-US").unwrap_err();

    assert_eq!(error.code(), "catalog_project_stale");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn localized_catalog_returns_coherent_metadata_with_camel_case_serialization() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-localized-catalog-metadata-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref()).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let expected_fingerprint = state
        .project_store
        .read()
        .unwrap()
        .node_registry
        .fingerprint()
        .to_string();

    let catalog =
        get_localized_node_catalog_from_state(&state, project_instance_id.clone(), "en-US")
            .unwrap();

    assert_eq!(
        catalog.project_instance_id.as_ref(),
        project_instance_id.as_str()
    );
    assert_eq!(catalog.registry_fingerprint.as_ref(), expected_fingerprint);
    assert_eq!(catalog.resource_publication_revision, 0);
    let value = serde_json::to_value(&catalog).unwrap();
    assert_eq!(value["projectInstanceId"], project_instance_id.as_str());
    assert_eq!(value["registryFingerprint"], expected_fingerprint);
    assert_eq!(value["resourcePublicationRevision"], 0);
    assert!(value.get("project_instance_id").is_none());
    assert!(value.get("registry_fingerprint").is_none());
    assert!(value.get("resource_publication_revision").is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn localized_catalog_returns_resources_from_the_same_coherent_snapshot() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-localized-catalog-resource-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let function_path = GraphResourcePath::new("functions/Sales Report.yssbi-function").unwrap();
    let mut project = ProjectData::new();
    project.graphs.insert(
        function_path.clone(),
        GraphResourceDocument::new("Sales Report", GraphDocumentKind::Function),
    );
    fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
    fixtures::write_graph(&project, root.to_string_lossy().as_ref(), &function_path).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let snapshot = state.catalog_snapshot(&project_instance_id).unwrap();
    let expected_fingerprint = snapshot.registry.fingerprint().to_string();
    let expected_revision = snapshot.resource_publication_revision;

    let catalog =
        get_localized_node_catalog_from_state(&state, project_instance_id.clone(), "zh-CN")
            .unwrap();

    assert_eq!(
        catalog.project_instance_id.as_ref(),
        project_instance_id.as_str()
    );
    assert_eq!(catalog.registry_fingerprint.as_ref(), expected_fingerprint);
    assert_eq!(catalog.resource_publication_revision, expected_revision);
    let resource = catalog
        .items
        .iter()
        .find(|item| item.resource_path.is_some())
        .expect("persisted function must be projected by the Catalog command");
    assert_eq!(resource.title.as_ref(), "Sales Report");
    assert_eq!(
        resource
            .resource_path
            .as_ref()
            .map(crate::node_system::catalog::CatalogResourcePath::as_str),
        Some(function_path.as_str())
    );
    assert!(matches!(
        resource.creation,
        NodeCreationDescriptor::ResourceBound { .. }
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn activation_final_publication_uses_only_constant_time_authority_checks() {
    let project_source = include_str!("../../../project/project_state/activation.rs");
    let publish_start = project_source
        .find("    pub(in crate::project) fn publish_project_activation(")
        .expect("activation publication must exist");
    let publish_end = project_source
        .rfind("\n}")
        .expect("activation publication region must be isolated");
    let publish = &project_source[publish_start..publish_end];

    assert!(publish.contains("authority_generation"));
    for forbidden in [
        "canonical_semantic_value",
        "serde_json::to_value",
        "current_data.clone()",
        "current_store.clone()",
        ".sort_unstable()",
        ".clear();",
        ".retain(",
        "*current_data =",
        "*current_store =",
        "*current_graph_revisions =",
        "*current_variable_revisions =",
        "*current_worksheet_revisions =",
        "*history =",
    ] {
        assert!(
            !publish.contains(forbidden),
            "activation publication contains size-dependent work: {forbidden}"
        );
    }
}
