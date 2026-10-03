//! Capture and rewrite graph references without interpreting ordinary text as resource identity.
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_document::{
    DocumentNode, DynamicMemberLocator, DynamicPortBinding, GraphDocument, GraphDocumentOperation,
    GraphDocumentPatch, GraphResourceKind, GraphResourcePath,
};
use yss_node_protocol::{ParameterEditorSpec, PortCardinality, ResourceDisplayKind};
use yss_node_registry::{NodeRegistry, StructuralNodeRole};

pub(super) fn capture_function_dependents(
    snapshot: &crate::project_writers::WriterSnapshot,
    changed: &GraphResourcePath,
    registry: &NodeRegistry,
) -> Result<Vec<String>, crate::ProjectOperationError> {
    let mut capture = FunctionDependencyCapture {
        changed,
        registry,
        callers: BTreeMap::new(),
        pending: BTreeSet::new(),
        affected: BTreeSet::from([changed.clone()]),
    };
    let mut visited = BTreeSet::new();
    for (path, graph) in &snapshot.data.graphs {
        visited.insert(path.clone());
        capture.record_document(path, &graph.document);
    }
    let root = snapshot.session.root.as_path();
    let mut index = None;
    let prepare_error =
        |error: crate::ProjectError| crate::ProjectOperationError::TransactionPrepareFailed {
            message: error.to_string(),
        };
    while let Some(path) = capture.pending.pop_first() {
        if !visited.insert(path.clone()) {
            continue;
        }
        if index.is_none() {
            index = Some(crate::scan_graph_resource_index(root).map_err(prepare_error)?);
        }
        let Some(entry) = index
            .as_ref()
            .and_then(|index| index.get_by_path(path.as_str()))
        else {
            continue;
        };
        // Reuse the captured path index. Reading a dependency must not install it as resident.
        let graph =
            crate::project_io::read_graph_document(&root.join(entry.path.as_str()), entry.kind)
                .map_err(prepare_error)?;
        capture.record_document(&path, &graph.document);
    }
    Ok(capture.affected_graphs())
}

// These reverse edges exist only while preparing one signature transaction.
struct FunctionDependencyCapture<'a> {
    changed: &'a GraphResourcePath,
    registry: &'a NodeRegistry,
    callers: BTreeMap<GraphResourcePath, BTreeSet<GraphResourcePath>>,
    pending: BTreeSet<GraphResourcePath>,
    affected: BTreeSet<GraphResourcePath>,
}

impl FunctionDependencyCapture<'_> {
    fn record_document(&mut self, caller: &GraphResourcePath, document: &GraphDocument) {
        for node in document.nodes.values() {
            let Some(registered) = self.registry.get(&node.node_type) else {
                continue;
            };
            let protocol = registered.protocol();
            let resolver_references = protocol.interface.ports.iter().filter_map(|port| {
                let PortCardinality::Derived { resolver } = &port.cardinality else {
                    return None;
                };
                StructuralNodeRole::for_interface_resolver(resolver.as_str())?
                    .reference_parameter(protocol)
            });
            let resource_parameters = protocol.parameters.iter().filter(|parameter| {
                matches!(
                    parameter.editor,
                    ParameterEditorSpec::Resource {
                        kind: ResourceDisplayKind::Function
                    }
                )
            });
            if registered
                .function_reference_parameter()
                .into_iter()
                .chain(resolver_references)
                .chain(resource_parameters)
                .any(|parameter| {
                    protocol
                        .parameters
                        .effective_text(&parameter.key, &node.parameters)
                        .and_then(|path| GraphResourcePath::new(path).ok())
                        .as_ref()
                        == Some(self.changed)
                })
            {
                self.affected.insert(caller.clone());
            }
            if registered.structural_role() == Some(StructuralNodeRole::Call)
                && let Some(target) = registered
                    .function_reference(&node.parameters)
                    .and_then(|path| GraphResourcePath::new(path).ok())
                    .filter(|path| path.kind() == GraphResourceKind::FunctionGraph)
            {
                self.callers
                    .entry(target.clone())
                    .or_default()
                    .insert(caller.clone());
                self.pending.insert(target);
            }
        }
    }

    fn affected_graphs(mut self) -> Vec<String> {
        let mut pending = self.affected.iter().cloned().collect::<Vec<_>>();
        while let Some(path) = pending.pop() {
            if let Some(callers) = self.callers.get(&path) {
                for caller in callers {
                    if self.affected.insert(caller.clone()) {
                        pending.push(caller.clone());
                    }
                }
            }
        }
        self.affected
            .into_iter()
            .map(|path| path.as_str().to_owned())
            .collect()
    }
}

fn remap_node(
    node: &mut DocumentNode,
    from: &GraphResourcePath,
    to: &GraphResourcePath,
    registry: &NodeRegistry,
) -> bool {
    let Some(registered) = registry.get(&node.node_type) else {
        return false;
    };
    let Some(parameter) = registered.function_reference_parameter() else {
        return false;
    };
    // Preserve stored references, including inactive fields in reversible history.
    // Only applicable defaults are current references and may become explicit overrides.
    let reference = match node.parameters.get(&parameter.key) {
        Some(value) => value.as_str(),
        None => registered.function_reference(&node.parameters),
    };
    if reference
        .and_then(|value| GraphResourcePath::new(value).ok())
        .as_ref()
        != Some(from)
    {
        return false;
    }
    node.parameters.insert(
        parameter.key.clone(),
        serde_json::Value::String(to.as_str().into()),
    );
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
    registry: &NodeRegistry,
) -> bool {
    let mut changed = false;
    for node in document.nodes.values_mut() {
        changed |= remap_node(node, from, to, registry);
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
    registry: &NodeRegistry,
) -> bool {
    let mut changed = false;
    for operation in &mut patch.operations {
        match operation {
            GraphDocumentOperation::InsertNode { node }
            | GraphDocumentOperation::RemoveNode { node } => {
                changed |= remap_node(node, from, to, registry);
            }
            GraphDocumentOperation::UpdateNode { before, after } => {
                changed |= remap_node(before, from, to, registry);
                changed |= remap_node(after, from, to, registry);
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
