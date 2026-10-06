use super::*;
use serde_json::json;
use yss_harness_contract::model::*;

fn read(
    f: &mut Fixture,
    input: MindReadInput,
    version: Option<ResourceVersion>,
) -> Result<MindReadResult> {
    match f.call(AutomationCapabilityRequest::ReadMind(MindReadRequest {
        input,
        version,
    }))? {
        AutomationCapabilityResult::MindRead(value) => Ok(value),
        _ => panic!("expected topic read"),
    }
}

#[test]
fn topic_tools_page_real_tree_content_and_commit_atomic_batches_with_fresh_copy_ids() {
    let mut f = Fixture::new();
    let resource = f.create(ResourceCreation::Mind {
        name: "Plan".into(),
    });
    let mind = MindResourceRef::new(resource.id.clone());
    let outline = MindReadInput::Outline(
        serde_json::from_value(json!({"mind": mind, "depth": 1, "limit": 2})).unwrap(),
    );
    let original = read(&mut f, outline.clone(), None).unwrap();
    let MindReadContent::Outline {
        root_topic_id: root,
        topic_count: 1,
        ..
    } = original.content
    else {
        panic!()
    };
    let publications = f.publications.len();
    // A later invalid parent rejects even the valid first item, before publication.
    let error = f
        .call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: original.version.clone(),
                edit: ResourceEdit::CreateTopics {
                    before_id: None,
                    topics: vec![
                        TopicCreation {
                            client_id: "valid".into(),
                            parent_id: root.clone(),
                            content: "unchanged".into(),
                            reference: None,
                        },
                        TopicCreation {
                            client_id: "bad".into(),
                            parent_id: "missing".into(),
                            content: "bad".into(),
                            reference: None,
                        },
                    ],
                },
            },
        ))
        .unwrap_err();
    assert_eq!(error.code, CapabilityFailureCode::InvalidRequest);
    assert_eq!(
        read(&mut f, outline.clone(), None).unwrap().version,
        original.version
    );
    assert_eq!(f.publications.len(), publications);

    let reference = MindResourceReference::Database {
        database_id: "unavailable-external-target".into(),
    };
    let created = f.edit(
        &resource,
        ResourceEdit::CreateTopics {
            before_id: None,
            topics: vec![
                TopicCreation {
                    client_id: "leaf".into(),
                    parent_id: "$branch".into(),
                    content: "中文🙂Leaf".into(),
                    reference: Some(reference.clone()),
                },
                TopicCreation {
                    client_id: "branch".into(),
                    parent_id: root.clone(),
                    content: "Branch".into(),
                    reference: None,
                },
            ],
        },
    );
    assert_eq!(f.publications.len(), publications + 1);
    let facts = created.mind_edit.as_ref().unwrap();
    assert!(facts.dirty);
    let branch = facts.created_topics["branch"].clone();
    let leaf = facts.created_topics["leaf"].clone();
    assert_ne!(leaf, "leaf");
    let first = read(&mut f, outline.clone(), None).unwrap();
    let MindReadContent::Outline {
        topic_count,
        subtree_count,
        topics,
        page,
        ..
    } = &first.content
    else {
        panic!()
    };
    assert_eq!((*topic_count, *subtree_count, page.total), (3, 3, Some(2)));
    assert_eq!(
        topics.iter().map(|v| &v.topic_id).collect::<Vec<_>>(),
        [&root, &branch]
    );
    assert!(first.dirty);
    let projected =
        model::capability_result(&AutomationCapabilityResult::MindRead(first.clone())).unwrap();
    assert!(projected["payload"].get("version").is_none());
    assert!(read(&mut f, outline.clone(), Some(original.version)).is_err());

    let found = read(
        &mut f,
        MindReadInput::Find(
            serde_json::from_value(json!({"mind": mind, "query":"LEAF", "rootTopicId": branch}))
                .unwrap(),
        ),
        Some(first.version.clone()),
    )
    .unwrap();
    let MindReadContent::Found { topics, page } = found.content else {
        panic!()
    };
    assert_eq!(page.total, Some(1));
    assert_eq!(topics[0].path, [root.clone(), branch.clone(), leaf.clone()]);
    assert!(topics[0].path_complete);
    let detail = MindReadInput::Topics(
        serde_json::from_value(
            json!({"mind": mind, "topicIds":[leaf], "contentOffset":1, "contentLimit":2}),
        )
        .unwrap(),
    );
    let inspected = read(&mut f, detail.clone(), Some(first.version.clone())).unwrap();
    let MindReadContent::Topics { topics } = inspected.content else {
        panic!()
    };
    assert_eq!(topics[0].content, "文🙂");
    assert_eq!(topics[0].content_page.next_offset, Some(3));
    assert_eq!(topics[0].reference, Some(reference.clone()));

    let before = f.inspect(&resource);
    let publications = f.publications.len();
    let rejected = f
        .call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: before.version.clone(),
                edit: ResourceEdit::UpdateTopics {
                    topics: vec![
                        TopicUpdate {
                            topic_id: root.clone(),
                            content: Some("must not commit".into()),
                            reference: None,
                        },
                        TopicUpdate {
                            topic_id: "missing".into(),
                            content: Some("bad".into()),
                            reference: None,
                        },
                    ],
                },
            },
        ))
        .unwrap_err();
    assert_eq!(rejected.code, CapabilityFailureCode::InvalidRequest);
    assert_eq!(f.inspect(&resource), before);
    assert_eq!(f.publications.len(), publications);
    let cycle = f
        .call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: before.version,
                edit: ResourceEdit::MoveTopics {
                    topics: vec![TopicMove {
                        topic_id: branch.clone(),
                        parent_id: leaf.clone(),
                        before_id: None,
                    }],
                },
            },
        ))
        .unwrap_err();
    assert_eq!(cycle.code, CapabilityFailureCode::InvalidRequest);
    assert_eq!(f.publications.len(), publications);
    let copied = f.edit(
        &resource,
        ResourceEdit::DuplicateTopics {
            topic_ids: vec![branch.clone()],
            parent_id: root.clone(),
            before_id: Some(branch.clone()),
        },
    );
    let copies = &copied.mind_edit.as_ref().unwrap().created_topics;
    assert_eq!(copies.len(), 2);
    assert_ne!(copies[&branch], branch);
    assert_ne!(copies[&leaf], leaf);
    let copied_leaf = copies[&leaf].clone();
    let copied_branch = copies[&branch].clone();
    let copied_detail = read(
        &mut f,
        MindReadInput::Topics(
            serde_json::from_value(json!({"mind":mind,"topicIds":[copied_leaf]})).unwrap(),
        ),
        None,
    )
    .unwrap();
    let MindReadContent::Topics { topics } = copied_detail.content else {
        panic!()
    };
    assert_eq!(topics[0].parent_id, Some(copied_branch.clone()));
    assert_eq!(topics[0].reference, Some(reference));
    let update: TopicUpdate =
        serde_json::from_value(json!({"topicId":copied_leaf,"reference":null})).unwrap();
    assert_eq!(update.reference, Some(None));
    f.edit(
        &resource,
        ResourceEdit::UpdateTopics {
            topics: vec![update],
        },
    );
    let updated = read(
        &mut f,
        MindReadInput::Topics(
            serde_json::from_value(json!({"mind":mind,"topicIds":[copied_leaf]})).unwrap(),
        ),
        None,
    )
    .unwrap();
    let MindReadContent::Topics { topics } = updated.content else {
        panic!()
    };
    assert!(topics[0].reference.is_none());
    assert_eq!(topics[0].content, "中文🙂Leaf");
    // Final relationships are validated together: the first move alone would form a cycle.
    f.edit(
        &resource,
        ResourceEdit::MoveTopics {
            topics: vec![
                TopicMove {
                    topic_id: branch.clone(),
                    parent_id: leaf.clone(),
                    before_id: None,
                },
                TopicMove {
                    topic_id: leaf.clone(),
                    parent_id: root.clone(),
                    before_id: None,
                },
            ],
        },
    );
    let deleted = f.edit(
        &resource,
        ResourceEdit::DeleteTopics {
            topic_ids: vec![copied_branch.clone()],
        },
    );
    let deleted_ids = &deleted.mind_edit.as_ref().unwrap().deleted_topic_ids;
    assert_eq!(deleted_ids.len(), 2);
    assert!(deleted_ids.contains(&copied_branch) && deleted_ids.contains(&copied_leaf));
    let current = read(&mut f, outline.clone(), None).unwrap();
    let forbidden = f
        .call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: current.version.clone(),
                edit: ResourceEdit::DeleteTopics {
                    topic_ids: vec![root],
                },
            },
        ))
        .unwrap_err();
    assert_eq!(forbidden.code, CapabilityFailureCode::InvalidRequest);
    assert_eq!(
        read(&mut f, outline.clone(), None).unwrap().version,
        current.version
    );
    f.save(&resource);
    assert!(!read(&mut f, outline, None).unwrap().dirty);
    let on_disk: MindDocument = serde_json::from_str(
        &std::fs::read_to_string(f.directory.join("project").join(&resource.id)).unwrap(),
    )
    .unwrap();
    on_disk.validate().unwrap();
    assert_eq!(on_disk.nodes.len(), 3);
    assert_eq!(
        on_disk
            .nodes
            .iter()
            .find(|v| v.id == branch)
            .unwrap()
            .parent_id,
        Some(leaf)
    );
}
