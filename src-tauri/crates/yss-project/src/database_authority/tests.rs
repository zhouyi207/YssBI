use super::*;
use yss_project_identity::{OperationId, ResourceRevision};

#[test]
fn declaration_reads_cannot_cross_project_activation_with_a_reused_database_id() {
    let mut declaration = DatabaseDecl {
        id: yss_database_contract::DatabaseId::from_existing("shared".into()),
        engine: yss_database_contract::DatabaseEngine::Dataset {},
        schema_version: 1,
        required: false,
        name: "Original".into(),
    };
    let mut initial = yss_project_model::ProjectData::new();
    initial
        .databases
        .insert("shared".into(), declaration.clone());
    let fixture = crate::fixtures::TempProject::activate("database-declaration-identity", initial);
    let project = fixture.state();
    let captured = project.capture_project_session().unwrap();
    assert_eq!(
        project
            .read_database_declaration(&captured.instance_id, "shared")
            .unwrap(),
        declaration
    );
    declaration.name = "Replacement".into();
    let mut replacement = yss_project_model::ProjectData::new();
    replacement
        .databases
        .insert("shared".into(), declaration.clone());
    project.activate_project_fixture(
        captured.root.as_path().to_string_lossy().into_owned(),
        replacement,
    );

    assert!(matches!(
        project.read_database_declaration(&captured.instance_id, "shared"),
        Err(ProjectDatabaseError::Project(
            ProjectOperationError::StaleProjectLifecycle { .. }
        ))
    ));
    let current = project.capture_project_session().unwrap();
    assert_eq!(
        project
            .read_database_declaration(&current.instance_id, "shared")
            .unwrap(),
        declaration
    );
}

#[test]
fn failed_database_delete_keeps_the_declaration_and_revision() {
    let fixture = crate::fixtures::TempProject::activate(
        "database-delete-overflow",
        yss_project_model::ProjectData::new(),
    );
    let project = fixture.state();
    let instance = project.capture_project_session().unwrap().instance_id;
    let declaration = DatabaseDecl {
        id: yss_database_contract::DatabaseId::from_existing("retained".into()),
        engine: yss_database_contract::DatabaseEngine::Dataset {},
        schema_version: 1,
        required: false,
        name: "Retained".into(),
    };
    project
        .commit_database_declaration_add(&instance, declaration.clone(), OperationId::new())
        .unwrap();
    project
        .mutation_publication
        .lock()
        .unwrap()
        .authority_generation = u64::MAX;
    let revision = project.database_authority_revisions.read().unwrap()["retained"];
    let snapshot = project.read_database_snapshot().unwrap();

    let result = project.commit_database_declaration_delete(
        &instance,
        "retained",
        ResourceRevision::new(revision),
        OperationId::new(),
    );

    assert!(matches!(
        result,
        Err(ProjectDatabaseError::Project(
            ProjectOperationError::AuthorityGenerationExhausted
        ))
    ));
    assert_eq!(
        project.get_data().unwrap().databases.get("retained"),
        Some(&declaration)
    );
    assert_eq!(
        project.database_authority_revisions.read().unwrap()["retained"],
        revision
    );
    project.revalidate_database_snapshot(&snapshot).unwrap();
}
