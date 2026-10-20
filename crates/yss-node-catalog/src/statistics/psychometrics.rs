//! Item-scale interfaces; numerical estimators live in SCI.
use super::*;
const ENTRIES: [(&str,); 4] = [
    ("reliability",),
    ("validity",),
    ("content_validity",),
    ("item_analysis",),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for (suffix,) in ENTRIES {
        let id = format!("yssbi.statistics.psychometrics.{suffix}");
        let mut ports = vec![
            bounded_user_data_input(
                "items",
                "Scored item",
                numeric_data_series_type(),
                if suffix == "content_validity" { 1 } else { 2 },
                None,
            )?,
            data_output("result", "Scale summary", report_type()?)?,
        ];
        let mut fields = vec![
            "item",
            "mean",
            "standard_deviation",
            "corrected_item_total_correlation",
            "alpha_if_deleted",
        ];
        if suffix == "content_validity" {
            fields = vec![
                "item",
                "relevant_experts",
                "item_cvi",
                "chance_agreement",
                "modified_kappa",
            ]
        }
        if suffix == "item_analysis" {
            fields.extend([
                "low_mean",
                "high_mean",
                "t_statistic",
                "degrees_of_freedom",
                "p_value",
            ])
        }
        if suffix != "validity" {
            ports.push(fixed_numeric_table(
                "item_statistics",
                "Item statistics",
                &fields,
            )?)
        }
        if suffix == "item_analysis" {
            ports.push(fixed_numeric_table(
                "scores",
                "Total scores and groups",
                &["observation", "total", "group"],
            )?)
        }
        let parameters = if suffix == "item_analysis" {
            let mut p = decimal_parameter("tail_fraction", "0.27")?;
            node_scoped_keys(&id, std::slice::from_mut(&mut p))?;
            vec![p]
        } else {
            vec![]
        };
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: None,
                    category_id: sid("statistics.psychometrics", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, ports, vec![], vec![])?,
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
