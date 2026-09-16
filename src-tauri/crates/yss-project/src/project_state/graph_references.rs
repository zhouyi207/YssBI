//! Rewrites graph references without interpreting ordinary text as resource identity.
use yss_graph_document::{
    DocumentNode, DynamicMemberLocator, DynamicPortBinding, GraphDocument, GraphDocumentOperation,
    GraphDocumentPatch, GraphResourcePath,
};

fn remap_node(node: &mut DocumentNode, from: &GraphResourcePath, to: &GraphResourcePath) -> bool {
    let parameter = match node.node_type.as_str() {
        "yssbi.project.function.call" => "target",
        "yssbi.project.function.entry" | "yssbi.project.function.return" => "function",
        _ => return false,
    };
    let Some(value) = node
        .parameters
        .iter_mut()
        .find_map(|(key, value)| (key.as_str() == parameter).then_some(value))
    else {
        return false;
    };
    if value
        .as_str()
        .and_then(|value| GraphResourcePath::new(value).ok())
        .as_ref()
        != Some(from)
    {
        return false;
    }
    *value = serde_json::Value::String(to.as_str().into());
    true
}

fn remap_binding(
    binding: &mut DynamicPortBinding,
    from: &GraphResourcePath,
    to: &GraphResourcePath,
) -> bool {
    let origin = match binding {
        DynamicPortBinding::Resolved { origin, .. } | DynamicPortBinding::Orphan { origin, .. } => {
            origin
        }
        DynamicPortBinding::UserCreated { .. } => return false,
    };
    if let DynamicMemberLocator::FunctionParameter { function, .. } = origin
        && function == from
    {
        *function = to.clone();
        return true;
    }
    false
}

pub(super) fn remap_document(
    document: &mut GraphDocument,
    from: &GraphResourcePath,
    to: &GraphResourcePath,
) -> bool {
    let mut changed = false;
    for node in document.nodes.values_mut() {
        changed |= remap_node(node, from, to);
    }
    for binding in document.port_bindings.values_mut() {
        changed |= remap_binding(binding, from, to);
    }
    changed
}

pub(super) fn remap_patch(
    patch: &mut GraphDocumentPatch,
    from: &GraphResourcePath,
    to: &GraphResourcePath,
) -> bool {
    let mut changed = false;
    for operation in &mut patch.operations {
        match operation {
            GraphDocumentOperation::InsertNode { node }
            | GraphDocumentOperation::RemoveNode { node } => {
                changed |= remap_node(node, from, to);
            }
            GraphDocumentOperation::UpdateNode { before, after } => {
                changed |= remap_node(before, from, to);
                changed |= remap_node(after, from, to);
            }
            GraphDocumentOperation::InsertPortBinding { binding, .. }
            | GraphDocumentOperation::RemovePortBinding { binding, .. } => {
                changed |= remap_binding(binding, from, to);
            }
            GraphDocumentOperation::SetConstant { .. }
            | GraphDocumentOperation::SetInputState { .. }
            | GraphDocumentOperation::InsertConnection { .. }
            | GraphDocumentOperation::RemoveConnection { .. } => {}
        }
    }
    changed
}
