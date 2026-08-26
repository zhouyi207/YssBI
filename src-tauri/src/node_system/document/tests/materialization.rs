use super::*;

#[test]
fn compilation_basis_token_preserves_the_complete_resolver_basis() {
    let basis = compilation_basis("events/main.yssbi-event", GraphRevision::new(3));
    let resource = CompilationResourceKey::new("schema/source");

    assert_eq!(basis.graph_path(), &graph_path("events/main.yssbi-event"));
    assert_eq!(basis.graph_revision(), GraphRevision::new(3));
    assert_eq!(basis.registry_fingerprint().as_bytes(), &[7; 32]);
    assert_eq!(
        basis
            .resource_versions()
            .get(&resource)
            .map(|value| value.as_str()),
        Some("v1")
    );
}

#[test]
fn projected_member_rejects_a_stale_compilation_basis() {
    let path = graph_path("events/main.yssbi-event");
    let source = node_id(530);
    let target = node_id(531);
    let mut document = GraphDocument::default();
    document.create_node(node(source)).unwrap();
    document.create_node(node(target)).unwrap();
    let mut store = RevisionedGraphStore::new(path.clone(), document);
    let before = store.document().clone();
    let member = projected_member("events/main.yssbi-event", GraphRevision::INITIAL, target);
    let authorization = authorization(member.clone());

    let result = store.apply_mutation(MutationRequest::new(
        ResourceKey::Graph(path),
        store.revision(),
        operation_id(532),
        GraphMutation::MaterializeProjectedMemberAndConnect {
            member,
            authorization,
            output: declared(source, "output"),
            order: None,
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::CompilationBasisStale { .. })
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn projected_member_rejects_authorization_for_another_member() {
    let path = graph_path("events/main.yssbi-event");
    let source = node_id(535);
    let target = node_id(536);
    let mut document = GraphDocument::default();
    document.create_node(node(source)).unwrap();
    document.create_node(node(target)).unwrap();
    let mut store = RevisionedGraphStore::new(path.clone(), document);
    let member = projected_member(
        "events/main.yssbi-event",
        store.revision().to_graph_revision(),
        target,
    );
    let other = ProjectedMemberRef::new(
        member.basis().clone(),
        target,
        member.template().clone(),
        PortDirection::Input,
        DynamicMemberLocator::SchemaField {
            source: SchemaSourceIdentity::new("source"),
            field: SchemaFieldIdentity::new("other"),
        },
        LastKnownPortMetadata::default(),
    );
    let before = store.document().clone();

    let result = store.apply_mutation(MutationRequest::new(
        ResourceKey::Graph(path),
        store.revision(),
        operation_id(537),
        GraphMutation::MaterializeProjectedMemberAndConnect {
            member,
            authorization: authorization(other),
            output: declared(source, "output"),
            order: None,
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::MaterializationUnauthorized)
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn projected_member_materialization_and_connection_commit_atomically() {
    let path = graph_path("events/main.yssbi-event");
    let resource = ResourceKey::Graph(path.clone());
    let source = node_id(540);
    let target = node_id(541);
    let mut document = GraphDocument::default();
    document.create_node(node(source)).unwrap();
    document.create_node(node(target)).unwrap();
    let mut store = RevisionedGraphStore::new(path, document);
    let before_revision = store.revision();
    let member = projected_member(
        "events/main.yssbi-event",
        before_revision.to_graph_revision(),
        target,
    );

    let event = store
        .apply_mutation(MutationRequest::new(
            resource.clone(),
            before_revision,
            operation_id(542),
            GraphMutation::MaterializeProjectedMemberAndConnect {
                authorization: authorization(member.clone()),
                member,
                output: declared(source, "output"),
                order: None,
            },
        ))
        .unwrap();

    assert_eq!(event.to_revision, before_revision.next());
    assert_eq!(store.document().port_bindings.len(), 1);
    assert_eq!(store.document().connections.len(), 1);
    let address = store.document().port_bindings.keys().next().unwrap();
    assert!(matches!(address.port, PortRef::Instance { .. }));
    assert_eq!(
        store.document().connections.values().next().unwrap().input,
        address.clone()
    );

    let invalid_source = node_id(543);
    let before_failed_request = store.document().clone();
    let member = projected_member(
        "events/main.yssbi-event",
        store.revision().to_graph_revision(),
        target,
    );
    let result = store.apply_mutation(MutationRequest::new(
        resource,
        store.revision(),
        operation_id(544),
        GraphMutation::MaterializeProjectedMemberAndConnect {
            authorization: authorization(member.clone()),
            member,
            output: declared(invalid_source, "output"),
            order: None,
        },
    ));

    assert!(matches!(result, Err(MutationConflict::Document(_))));
    assert_eq!(store.document(), &before_failed_request);
}
