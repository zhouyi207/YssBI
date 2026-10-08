use super::*;

pub(super) fn set_node_label(
    document: &GraphDocument,
    node_id: NodeId,
    label: Option<String>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let before = document.nodes.get(&node_id).cloned().ok_or_else(|| {
        editor_error(
            EditorMutationErrorCode::GraphNodeNotFound,
            "label target does not exist",
        )
    })?;
    if label.as_ref().is_some_and(|label| label.len() > 1024) {
        return Err(invalid_editor_mutation("node label exceeds 1024 bytes"));
    }
    let mut after = before.clone();
    after.user_label = label;
    Ok(vec![GraphDocumentOperation::UpdateNode { before, after }])
}

pub(super) fn set_port_counts(
    document: &GraphDocument,
    registry: &NodeRegistry,
    node_id: NodeId,
    requested: yss_node_protocol::InitialPortCounts,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let node = document.nodes.get(&node_id).ok_or_else(|| {
        editor_error(
            EditorMutationErrorCode::GraphNodeNotFound,
            "pin count target does not exist",
        )
    })?;
    let protocol = registry
        .protocol(&node.node_type)
        .ok_or_else(|| invalid_editor_mutation("unknown node protocol"))?;
    // Reuse creation's fixed/derived rejection, bounds and coupled-template validation.
    // Its defaults are ignored for templates that were not explicitly requested.
    let totals = protocol
        .interface
        .initial_port_counts(&requested)
        .map_err(|error| invalid_editor_mutation(error.to_string()))?;
    let mut seen = BTreeSet::new();
    let mut staged = document.clone();
    let mut operations = Vec::new();
    for template in requested.keys() {
        if !seen.insert(template.clone()) {
            continue;
        }
        let templates = protocol
            .interface
            .member_group_for_template(template)
            .map_or(std::slice::from_ref(template), |group| {
                group.templates.as_ref()
            });
        seen.extend(templates.iter().cloned());
        let target = usize::from(totals[template]);
        let members = user_created_member_sequence(&staged, node_id, templates);
        // Keep the earliest existing members and remove from the end. Removal reuses
        // the owner's group, literal and incident-connection cleanup rules.
        for member in members.iter().skip(target).rev() {
            let patch = GraphDocumentPatch::new(remove_port_instance_operations(
                &staged,
                registry,
                PortAddress::instance(node_id, template.clone(), *member),
            )?);
            staged = prepare_graph_document_patch(staged, &patch)?;
            operations.extend(patch.operations);
        }
        for _ in members.len()..target {
            let patch = GraphDocumentPatch::new(add_port_instance_operations(
                &staged,
                registry,
                node_id,
                template.clone(),
                PortPlacement::Append,
            )?);
            staged = prepare_graph_document_patch(staged, &patch)?;
            operations.extend(patch.operations);
        }
    }
    Ok(operations)
}
