//! Observed-variable paths, mediation and interaction interfaces.
use super::*;
mod mediation;
mod moderation;
mod parameters;
mod recursive;
pub(super) fn implemented(id: &str) -> bool {
    matches!(
        id,
        "yssbi.statistics.workflow.moderation"
            | "yssbi.statistics.workflow.moderation_advanced"
            | "yssbi.statistics.workflow.mediation"
            | "yssbi.statistics.workflow.moderated_mediation"
            | "yssbi.statistics.sem.path"
    )
}
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    moderation::append(fragment)?;
    mediation::append(fragment)?;
    recursive::append(fragment)
}
fn append_node(
    fragment: &mut ProviderFragment,
    id: &str,
    en: &'static str,
    zh: &'static str,
    ports: Vec<PortSpec>,
    mut parameters: Vec<Parameter>,
) -> Result<(), BuiltinAssemblyError> {
    parameters::localize(fragment, id, &mut parameters)?;
    fragment.nodes.push(leaf(
        NodeProtocol {
            type_id: sid(id, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(id, "title")?,
                documentation_key: Some(node_key(id, "documentation")?),
                aliases_key: None,
                category_id: sid("statistics.psychometrics", NodeCategoryId::new)?,
                icon_id: sid("builtin.statistics", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(id, ports, vec![], vec![])?,
            parameters: assembled_parameters(id, parameters)?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: execution(),
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        },
        id,
    ));
    for (locale, title) in [("en-US", en), ("zh-CN", zh)] {
        fragment.messages.extend([
            (locale, node_key_text(id, "title"), Text(title)),
            (locale, node_key_text(id, "documentation"), Text(title)),
        ]);
    }
    Ok(())
}
