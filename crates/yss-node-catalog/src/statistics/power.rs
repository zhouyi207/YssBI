//! Prospective scalar power interfaces with model-specific units and assumptions.
use super::*;
mod parameters;
const METHODS: &[&str] = &[
    "principles",
    "mean_difference",
    "paired",
    "variance",
    "proportion",
    "proportion_difference",
    "correlation",
    "anova",
    "linear_regression",
    "generalized_model",
    "logistic",
    "cox",
    "logrank",
    "cluster_randomized",
    "noninferiority",
    "equivalence",
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &method in METHODS {
        let id = format!("yssbi.statistics.power.{method}");
        let parameters = parameters::build(&id, method)?;
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: None,
                    category_id: sid("statistics.power", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(
                    &id,
                    vec![data_output(
                        "result",
                        "Power and sample size",
                        report_type()?,
                    )?],
                    vec![],
                    vec![],
                )?,
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
