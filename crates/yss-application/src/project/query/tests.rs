use super::*;
use crate::session::{
    ApplicationSessionEpoch, ApplicationSessionSlot, NodeComponents,
    build_current_project_candidate,
};
use yss_project::{ProjectState, docs::DocCommand};
use yss_project_identity::OperationId;

#[test]
fn resource_reveal_uses_the_requested_project_and_an_existing_native_path() {
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
    let identity = application
        .capture_session()
        .unwrap()
        .project_instance_id()
        .clone();
    let snapshot = application
        .apply_doc_command(
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
