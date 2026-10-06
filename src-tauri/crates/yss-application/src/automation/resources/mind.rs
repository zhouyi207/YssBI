//! Harness topic projection and command translation over the canonical Mind owner.
use super::*;
use std::collections::BTreeMap;
use yss_project_model::file::FileContent;
use yss_project_model::mind::{MindEdit, MindNode, MindNodeMove, MindReference};

mod read;
pub(in crate::automation) use read::read_mind;

pub(super) fn edit_mind(
    application: &ApplicationState,
    session: &ApplicationSession,
    resource: &ProjectResourceRef,
    version: &ResourceVersion,
    edit: ResourceEdit,
    control: &CapabilityControl,
) -> Result<(CommittedResourceMutation, MindEditReceipt)> {
    let project = session.project_instance_id().clone();
    let path = mind_path(resource)?;
    let snapshot = application
        .read_mind(project.clone(), path.clone())
        .map_err(file_error)?;
    if snapshot.version != file_version(version)? {
        return Err(conflict());
    }
    let (edits, created_topics) = prepare_edit(&snapshot.content, edit, control)?;
    // Determine the exact affected/deleted range and reject an oversized receipt before commit.
    // Project repeats validation against this captured version when it stages the actual command.
    let mut candidate = snapshot.content.clone();
    for edit in &edits {
        control.check()?;
        candidate
            .apply(edit.clone())
            .map_err(|_| invalid("topics"))?;
    }
    let before = snapshot
        .content
        .nodes
        .iter()
        .map(|node| (&node.id, node))
        .collect::<BTreeMap<_, _>>();
    let after = candidate
        .nodes
        .iter()
        .map(|node| (&node.id, node))
        .collect::<BTreeMap<_, _>>();
    let mut receipt = MindEditReceipt {
        created_topics,
        affected_topic_ids: candidate
            .nodes
            .iter()
            .filter(|node| before.get(&node.id).is_none_or(|old| *old != *node))
            .map(|node| node.id.clone())
            .collect(),
        deleted_topic_ids: snapshot
            .content
            .nodes
            .iter()
            .filter(|node| !after.contains_key(&node.id))
            .map(|node| node.id.clone())
            .collect(),
        dirty: true,
    };
    // A same-parent reorder changes order, not node values. Include all requested move targets.
    for edit in &edits {
        if let MindEdit::MoveNodes { moves } = edit {
            for item in moves {
                if !receipt.affected_topic_ids.contains(&item.node_id) {
                    receipt.affected_topic_ids.push(item.node_id.clone());
                }
            }
        }
    }
    if serde_json::to_vec(&receipt)
        .map_err(|_| invalid("topics"))?
        .len()
        > MAX_CAPABILITY_RESULT_BYTES - 65_536
    {
        return Err(
            CapabilityFailure::new(CapabilityFailureCode::ResultTooLarge).with_detail(
                "nextStep",
                "Use a smaller subtree batch so the exact topic mapping fits in one receipt.",
            ),
        );
    }
    control.check()?;
    let result = application
        .apply_mind_command(
            project,
            OperationId::new(),
            FileCommand::Edit {
                path,
                version: snapshot.version,
                edits,
            },
        )
        .map_err(file_error)?;
    receipt.dirty = result.snapshot.as_ref().ok_or_else(unavailable)?.dirty;
    Ok((result.mutation, receipt))
}

fn prepare_edit(
    document: &MindDocument,
    edit: ResourceEdit,
    control: &CapabilityControl,
) -> Result<(Vec<MindEdit>, BTreeMap<String, String>)> {
    let mut created = BTreeMap::new();
    let edits = match edit {
        ResourceEdit::CreateTopics { topics, before_id } => {
            for topic in &topics {
                if created
                    .insert(topic.client_id.clone(), uuid::Uuid::new_v4().to_string())
                    .is_some()
                {
                    return Err(invalid("clientId"));
                }
            }
            let mut nodes = Vec::with_capacity(topics.len());
            for topic in topics {
                control.check()?;
                let parent_id = match topic.parent_id.strip_prefix('$') {
                    Some(alias) => created
                        .get(alias)
                        .cloned()
                        .ok_or_else(|| invalid("parentId"))?,
                    None => topic.parent_id,
                };
                nodes.push(MindNode {
                    id: created[&topic.client_id].clone(),
                    parent_id: Some(parent_id),
                    content: topic.content,
                    reference: topic.reference.map(reference_from_contract).transpose()?,
                });
            }
            vec![MindEdit::AddNodes { nodes, before_id }]
        }
        ResourceEdit::UpdateTopics { topics } => {
            let mut edits = Vec::new();
            for topic in topics {
                if let Some(content) = topic.content {
                    edits.push(MindEdit::SetContent {
                        node_id: topic.topic_id.clone(),
                        content,
                    });
                }
                if let Some(reference) = topic.reference {
                    edits.push(MindEdit::SetReference {
                        node_id: topic.topic_id,
                        reference: reference.map(reference_from_contract).transpose()?,
                    });
                }
            }
            edits
        }
        ResourceEdit::MoveTopics { topics } => vec![MindEdit::MoveNodes {
            moves: topics
                .into_iter()
                .map(|topic| MindNodeMove {
                    node_id: topic.topic_id,
                    parent_id: topic.parent_id,
                    before_id: topic.before_id,
                })
                .collect(),
        }],
        ResourceEdit::DeleteTopics { topic_ids } => vec![MindEdit::RemoveNodes {
            node_ids: topic_ids,
        }],
        ResourceEdit::DuplicateTopics {
            topic_ids,
            parent_id,
            before_id,
        } => {
            let prepared = document
                .prepare_subtree_copy(&topic_ids, &parent_id, before_id.as_deref(), &mut || {
                    uuid::Uuid::new_v4().to_string()
                })
                .map_err(|_| invalid("topics"))?;
            created = prepared.created;
            vec![prepared.edit]
        }
        _ => return Err(invalid("edit.kind")),
    };
    Ok((edits, created))
}

pub(super) fn reference_to_contract(reference: MindReference) -> MindResourceReference {
    match reference {
        MindReference::Resource { path } => MindResourceReference::Resource { path },
        MindReference::GraphNode { path, node_id } => MindResourceReference::GraphNode {
            path: path.as_str().into(),
            node_id,
        },
        MindReference::Database { database_id } => MindResourceReference::Database { database_id },
    }
}

fn reference_from_contract(reference: MindResourceReference) -> Result<MindReference> {
    Ok(match reference {
        MindResourceReference::Resource { path } => MindReference::Resource { path },
        MindResourceReference::GraphNode { path, node_id } => MindReference::GraphNode {
            path: GraphResourcePath::new(&path).map_err(|_| invalid("reference.path"))?,
            node_id,
        },
        MindResourceReference::Database { database_id } => MindReference::Database { database_id },
    })
}
