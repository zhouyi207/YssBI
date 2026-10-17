use super::*;
use crate::session::{
    ApplicationSessionEpoch, ApplicationSessionSlot, NodeComponents,
    build_current_project_candidate,
};
use yss_project::{ProjectState, docs::DocCommand};
use yss_project_identity::OperationId;

fn query_application() -> (tempfile::TempDir, ApplicationState) {
    let directory = tempfile::tempdir().unwrap();
    let data = yss_project_model::ProjectData::new();
    yss_project::fixtures::write_project(&data, directory.path().to_str().unwrap()).unwrap();
    let project = ProjectState::new();
    project.activate_project_fixture(directory.path().to_str().unwrap().into(), data);
    let nodes = NodeComponents::builtins().unwrap();
    let candidate = build_current_project_candidate(
        ApplicationSessionEpoch::INITIAL,
        Arc::new(project),
        std::iter::empty(),
        &nodes,
    )
    .unwrap();
    let application = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(nodes)));
    application.install_candidate(candidate).unwrap();
    (directory, application)
}

#[test]
fn raw_project_index_tracks_committed_resources_and_rejects_foreign_projects() {
    let (_directory, application) = query_application();
    let identity = application
        .capture_session()
        .unwrap()
        .project_instance_id()
        .clone();
    let initial = application.query_project_index(identity.clone()).unwrap();
    assert!(initial.docs.is_empty());

    let created = application
        .apply_file_command(
            identity.clone(),
            OperationId::new(),
            DocCommand::Create {
                name: "Methods 方法".into(),
            },
        )
        .unwrap();
    let index = application.query_project_index(identity.clone()).unwrap();
    assert_eq!(index.project_instance_id, identity.as_str());
    assert_eq!(
        index.publication_revision,
        created.mutation.publication_revision
    );
    assert!(index.publication_revision > initial.publication_revision);
    assert_eq!(index.docs.len(), 1);

    let foreign = ProjectInstanceId::from_existing("previous-project".into());
    assert!(matches!(
        application.query_project_index(foreign.clone()),
        Err(ProjectQueryApplicationError::ProjectIdentityMismatch { requested })
            if requested == foreign
    ));
    assert!(matches!(
        application.query_project_presentation(foreign.clone(), "en-US"),
        Err(ProjectQueryApplicationError::ProjectIdentityMismatch { requested })
            if requested == foreign
    ));
}

#[test]
fn project_query_rejects_a_publication_changed_during_projection() {
    let (_directory, application) = query_application();
    let identity = application
        .capture_session()
        .unwrap()
        .project_instance_id()
        .clone();
    let mut committed_revision = 0;
    let result = application.query_project_index_with(identity.clone(), |_, index| {
        committed_revision = application
            .apply_file_command(
                identity.clone(),
                OperationId::new(),
                DocCommand::Create {
                    name: "Changed during query".into(),
                },
            )
            .unwrap()
            .mutation
            .publication_revision;
        Ok(index)
    });
    assert!(matches!(
        result,
        Err(ProjectQueryApplicationError::Project(
            ProjectOperationError::CatalogResourceStale { .. }
        ))
    ));
    assert_eq!(
        application
            .query_project_index(identity)
            .unwrap()
            .publication_revision,
        committed_revision
    );
}

#[test]
fn resource_reveal_uses_the_requested_project_and_an_existing_native_path() {
    let (directory, application) = query_application();
    let identity = application
        .capture_session()
        .unwrap()
        .project_instance_id()
        .clone();
    let snapshot = application
        .apply_file_command(
            identity.clone(),
            OperationId::new(),
            DocCommand::Create {
                name: "Methods 方法".into(),
            },
        )
        .unwrap()
        .snapshot
        .unwrap();
    let request = RevealProjectResourceRequest::Doc {
        path: snapshot.path.clone(),
    };
    let expected = directory.path().join(snapshot.path.as_str());
    assert_eq!(
        application
            .reveal_project_resource(identity.clone(), request.clone())
            .unwrap(),
        expected
    );

    let foreign = ProjectInstanceId::from_existing("previous-project".into());
    assert!(
        matches!(application.reveal_project_resource(foreign.clone(), request.clone()), Err(ProjectQueryApplicationError::ProjectIdentityMismatch { requested }) if requested == foreign)
    );

    std::fs::remove_file(expected).unwrap();
    assert!(matches!(
        application.reveal_project_resource(identity, request),
        Err(ProjectQueryApplicationError::ResourceNotFound)
    ));
}
