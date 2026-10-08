//! Decision node declarations split by scoring and system-evaluation responsibilities.
use super::*;
mod conjoint;
mod experts;
mod market;
mod matrices;
mod parameters;
mod preferences;
mod ranking;
mod systems;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    ranking::append(fragment)?;
    systems::append(fragment)?;
    preferences::append(fragment)?;
    market::append(fragment)?;
    matrices::append(fragment)?;
    experts::append(fragment)?;
    conjoint::append(fragment)
}
fn emit(
    fragment: &mut ProviderFragment,
    suffix: &str,
    en: &'static str,
    zh: &'static str,
    ports: Vec<PortSpec>,
    mut parameters: Vec<Parameter>,
) -> Result<(), BuiltinAssemblyError> {
    let id = format!("yssbi.statistics.{suffix}");
    parameters::localize(fragment, &id, &mut parameters)?;
    fragment.nodes.push(leaf(
        NodeProtocol {
            type_id: sid(id.as_str(), NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(&id, "title")?,
                documentation_key: Some(node_key(&id, "documentation")?),
                aliases_key: None,
                category_id: sid("statistics.decision", NodeCategoryId::new)?,
                icon_id: sid("builtin.statistics", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(&id, ports, vec![], vec![])?,
            parameters: assembled_parameters(&id, parameters)?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: execution(),
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        },
        &id,
    ));
    for (locale, title) in [("en-US", en), ("zh-CN", zh)] {
        fragment.messages.extend([
            (locale, node_key_text(&id, "title"), Text(title)),
            (locale, node_key_text(&id, "documentation"), Text(title)),
        ]);
    }
    Ok(())
}
