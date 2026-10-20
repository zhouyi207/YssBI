//! Study summaries, inverse-variance analyses and meta plots.
use super::*;
mod interface;
mod parameters;

const NODES: &[&str] = &[
    "meta.continuous",
    "meta.binary",
    "meta.single_proportion",
    "meta.mean",
    "meta.correlation",
    "meta.or_hr",
    "meta.combine_p",
    "meta.inverse_variance",
    "meta.fixed_effect",
    "meta.random_effect",
    "meta.cochran_q",
    "meta.i_squared",
    "meta.tau_squared",
    "meta.regression",
    "meta.egger",
    "meta.begg",
    "meta.leave_one_out",
    "meta.sensitivity",
    "plot.forest",
    "plot.funnel",
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &suffix in NODES {
        let id = format!("yssbi.statistics.{suffix}");
        let method = suffix.rsplit_once('.').map_or(suffix, |(_, method)| method);
        let parameters = parameters::build(&id, method)?;
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: None,
                    category_id: sid("statistics.meta", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, interface::ports(method)?, vec![], vec![])?,
                parameters: assembled_parameters(&id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: pure(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            &id,
        ));
    }
    Ok(())
}
