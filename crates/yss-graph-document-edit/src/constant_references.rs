use std::collections::BTreeMap;
use yss_graph_document::ConstantId;
use yss_node_protocol::{
    Parameter, ParameterEditorSpec, ParameterKey, ParameterValues, Parameters,
};

/// References whose identities must survive copying a node's stored parameters.
/// Retain explicit references even in hidden fields; only applicable defaults are captured.
pub fn constant_references_for_copy<'a>(
    parameters: &'a Parameters,
    values: &'a ParameterValues,
) -> impl Iterator<Item = (&'a ParameterKey, ConstantId)> + 'a {
    parameters.iter().filter_map(move |parameter| {
        copied_reference(parameters, parameter, values).map(|id| (&parameter.key, id))
    })
}

pub fn remap_copied_constant_references(
    parameters: &Parameters,
    values: &mut ParameterValues,
    remapped: &BTreeMap<ConstantId, ConstantId>,
) {
    // Evaluate conditions against the source values before replacing any reference.
    let replacements = constant_references_for_copy(parameters, values)
        .filter_map(|(key, id)| remapped.get(&id).map(|id| (key.clone(), *id)))
        .collect::<Vec<_>>();
    for (key, id) in replacements {
        values.insert(key, id.to_string().into());
    }
}

fn copied_reference(
    parameters: &Parameters,
    parameter: &Parameter,
    values: &ParameterValues,
) -> Option<ConstantId> {
    if !matches!(parameter.editor, ParameterEditorSpec::GraphConstant) {
        return None;
    }
    let reference = match values.get(&parameter.key) {
        Some(value) => value.as_str(),
        None => parameters.effective_text(&parameter.key, values),
    }?;
    reference.parse().ok()
}
