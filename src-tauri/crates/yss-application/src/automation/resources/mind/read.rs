use super::*;
use yss_harness_contract::model::MindReadInput;

pub(in crate::automation) fn read_mind(
    application: &ApplicationState,
    session: &ApplicationSession,
    request: MindReadRequest,
    control: &CapabilityControl,
) -> Result<MindReadResult> {
    let resource = request.input.mind().resource();
    let snapshot = application
        .read_mind(session.project_instance_id().clone(), mind_path(&resource)?)
        .map_err(file_error)?;
    let version = ResourceVersion {
        revision: snapshot.version.revision.get(),
        session_id: Some(snapshot.version.session_id.clone()),
    };
    if request
        .version
        .as_ref()
        .is_some_and(|expected| expected != &version)
    {
        return Err(conflict());
    }
    let tree = snapshot.content.tree().map_err(|_| unavailable())?;
    let content = match &request.input {
        MindReadInput::Outline(value) => {
            let found = tree
                .outline(
                    value.root_topic_id.as_deref(),
                    value.depth,
                    value.offset,
                    value.limit,
                )
                .map_err(|_| invalid("rootTopicId"))?;
            let topics = found
                .entries
                .iter()
                .map(|entry| summary(entry.node, entry.depth, entry.child_count))
                .collect::<Vec<_>>();
            MindReadContent::Outline {
                root_topic_id: value
                    .root_topic_id
                    .clone()
                    .unwrap_or_else(|| snapshot.content.root_id.clone()),
                topic_count: snapshot.content.nodes.len(),
                subtree_count: found.subtree_count,
                page: InspectionPage::known(value.offset, topics.len(), found.total),
                topics,
            }
        }
        MindReadInput::Find(value) => {
            let found = tree
                .find(
                    value.root_topic_id.as_deref(),
                    &value.query,
                    value.offset,
                    value.limit,
                )
                .map_err(|_| invalid("rootTopicId"))?;
            let topics = found
                .topics
                .into_iter()
                .map(|detail| MindTopicMatch {
                    topic: summary(
                        detail.node,
                        detail.path.len().saturating_sub(1),
                        detail.child_ids.len(),
                    ),
                    path_complete: detail.path.len() <= 32,
                    path: detail
                        .path
                        .iter()
                        .skip(detail.path.len().saturating_sub(32))
                        .map(|id| (*id).to_owned())
                        .collect(),
                })
                .collect::<Vec<_>>();
            MindReadContent::Found {
                page: InspectionPage::known(value.offset, topics.len(), found.total),
                topics,
            }
        }
        MindReadInput::Topics(value) => {
            let topics = tree
                .inspect(&value.topic_ids)
                .map_err(|_| invalid("topicIds"))?
                .into_iter()
                .map(|detail| {
                    let content: String = detail
                        .node
                        .content
                        .chars()
                        .skip(value.content_offset)
                        .take(value.content_limit)
                        .collect();
                    let child_ids = detail
                        .child_ids
                        .iter()
                        .skip(value.children_offset)
                        .take(value.children_limit)
                        .map(|id| (*id).to_owned())
                        .collect::<Vec<_>>();
                    MindTopicInspection {
                        topic_id: detail.node.id.clone(),
                        parent_id: detail.node.parent_id.clone(),
                        content_page: InspectionPage::known(
                            value.content_offset,
                            content.chars().count(),
                            detail.node.content.chars().count(),
                        ),
                        content,
                        children_page: InspectionPage::known(
                            value.children_offset,
                            child_ids.len(),
                            detail.child_ids.len(),
                        ),
                        child_ids,
                        reference: detail.node.reference.clone().map(reference_to_contract),
                    }
                })
                .collect();
            MindReadContent::Topics { topics }
        }
    };
    control.check()?;
    check_version(session, &resource, &version)?;
    Ok(MindReadResult {
        mind: request.input.mind().clone(),
        version,
        dirty: snapshot.dirty,
        content,
    })
}

fn summary(node: &MindNode, depth: usize, child_count: usize) -> MindTopicSummary {
    MindTopicSummary {
        topic_id: node.id.clone(),
        parent_id: node.parent_id.clone(),
        summary: node.content.chars().take(160).collect(),
        content_length: node.content.chars().count(),
        depth,
        child_count,
    }
}
