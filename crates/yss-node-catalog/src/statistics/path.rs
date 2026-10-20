//! Observed-variable paths, mediation and interaction interfaces.
use super::*;
use crate::builtin::add_node_messages;
mod mediation;
mod moderation;
mod parameters;
mod recursive;
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
                aliases_key: None,
                category_id: sid("statistics.psychometrics", NodeCategoryId::new)?,
                icon_id: sid("builtin.statistics", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(id, ports, vec![], vec![])?,
            parameters: assembled_parameters(id, parameters)?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: pure(),
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        },
        id,
    ));
    add_node_messages(&mut fragment.messages, id, [en, zh], None);
    Ok(())
}
