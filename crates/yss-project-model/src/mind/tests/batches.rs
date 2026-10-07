use super::*;

fn document() -> MindDocument {
    let mut document = MindDocument::new("Analysis", &mut || "root".into());
    document
        .apply(MindEdit::AddNodes {
            // Child-first input is legal: validate the final batch, not each prefix.
            nodes: vec![
                MindNode {
                    id: "a1".into(),
                    parent_id: Some("a".into()),
                    content: "Result alpha".into(),
                    reference: Some(MindReference::Database {
                        database_id: "missing-database".into(),
                    }),
                },
                MindNode {
                    id: "b".into(),
                    parent_id: Some("root".into()),
                    content: "Result beta".into(),
                    reference: None,
                },
                MindNode {
                    id: "a".into(),
                    parent_id: Some("root".into()),
                    content: "Methods".into(),
                    reference: None,
                },
            ],
            before_id: None,
        })
        .unwrap();
    document
}

#[test]
fn local_tree_queries_preserve_sibling_order_depth_and_ancestor_paths() {
    let document = document();
    let tree = document.tree().unwrap();
    let page = tree.outline(None, 1, 1, 1).unwrap();
    assert_eq!((page.total, page.subtree_count), (3, 4));
    assert_eq!(page.entries[0].node.id, "b");
    assert_eq!(page.entries[0].depth, 1);
    let page = tree.outline(None, 1, 2, 1).unwrap();
    assert_eq!(page.entries[0].node.id, "a");
    assert_eq!(page.entries[0].child_count, 1);
    let page = tree.outline(Some("a"), 0, 0, 10).unwrap();
    assert_eq!((page.total, page.subtree_count), (1, 2));
    assert_eq!(page.entries[0].depth, 0);
    let matching = tree.find(None, "RESULT", 1, 1).unwrap();
    assert_eq!(matching.total, 2);
    assert_eq!(matching.topics[0].path, ["root", "a", "a1"]);
    let scoped = tree.find(Some("b"), "alpha", 0, 10).unwrap();
    assert_eq!(scoped.total, 0);
    let topics = tree.inspect(&["a".into(), "a1".into()]).unwrap();
    assert_eq!(topics[0].child_ids, ["a1"]);
    assert_eq!(topics[1].node.reference, document.nodes[1].reference);
    assert!(tree.inspect(&["a".into(), "missing".into()]).is_err());
    assert!(tree.inspect(&["a".into(), "a".into()]).is_err());
    assert!(tree.outline(Some("missing"), 1, 0, 1).is_err());
    assert!(
        tree.outline(None, 2, usize::MAX, 1)
            .unwrap()
            .entries
            .is_empty()
    );
}

#[test]
fn topic_batches_copy_fresh_identities_and_validate_final_parent_relationships() {
    let mut document = document();
    let mut next = 0;
    let copy = document
        .prepare_subtree_copy(&["a".into()], "root", Some("b"), &mut || {
            next += 1;
            format!("copy-{next}")
        })
        .unwrap();
    assert_eq!(copy.created.len(), 2);
    let copy_root = copy.created["a"].clone();
    let copy_child = copy.created["a1"].clone();
    document.apply(copy.edit).unwrap();
    let tree = document.tree().unwrap();
    let topics = tree.inspect(&["root".into(), copy_child.clone()]).unwrap();
    assert_eq!(topics[0].child_ids, [copy_root.as_str(), "b", "a"]);
    assert_eq!(
        topics[1].node.parent_id.as_deref(),
        Some(copy_root.as_str())
    );
    assert_eq!(topics[1].node.reference, tree.node("a1").unwrap().reference);
    assert!(
        document
            .prepare_subtree_copy(&["a".into(), "a1".into()], "root", None, &mut || "x".into())
            .is_err()
    );
    assert!(
        document
            .prepare_subtree_copy(&["a".into()], "root", None, &mut || "a".into())
            .is_err()
    );
    assert!(
        document
            .prepare_subtree_copy(&["a".into()], "b", Some("a1"), &mut || "x".into())
            .is_err()
    );
    // The first parent change alone would cycle; the final batch is a valid reversal.
    document
        .apply(MindEdit::MoveNodes {
            moves: vec![
                MindNodeMove {
                    node_id: "a".into(),
                    parent_id: "a1".into(),
                    before_id: None,
                },
                MindNodeMove {
                    node_id: "a1".into(),
                    parent_id: "root".into(),
                    before_id: Some("b".into()),
                },
            ],
        })
        .unwrap();
    assert_eq!(
        document.tree().unwrap().inspect(&["a".into()]).unwrap()[0].path,
        ["root", "a1", "a"]
    );
    let mut candidate = document.clone();
    assert!(
        candidate
            .apply(MindEdit::MoveNodes {
                moves: vec![MindNodeMove {
                    node_id: "a1".into(),
                    parent_id: "a".into(),
                    before_id: None
                }]
            })
            .is_err()
    );
    document
        .apply(MindEdit::RemoveNodes {
            node_ids: vec![copy_root.clone(), copy_child],
        })
        .unwrap();
    assert_eq!(document.nodes.len(), 4);
    assert!(document.tree().unwrap().node(&copy_root).is_err());
    assert!(
        document
            .apply(MindEdit::RemoveNodes {
                node_ids: vec!["root".into()]
            })
            .is_err()
    );
}
