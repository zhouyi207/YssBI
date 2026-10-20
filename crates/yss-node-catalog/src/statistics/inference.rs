//! Explicit estimand/SE intervals and independent-group post-hoc contrasts.
use super::*;
use yss_data_contract::SemanticType;
const NODES: &[&str] = &[
    "inference.confidence_interval",
    "posthoc.multiple_comparisons",
    "inference.cluster_robust",
    "postestimation.adjusted_predictions",
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &suffix in NODES {
        let id = format!("yssbi.statistics.{suffix}");
        let interval = suffix == "inference.confidence_interval";
        let cluster = suffix == "inference.cluster_robust";
        let prediction = suffix == "postestimation.adjusted_predictions";
        let mut ports = if cluster {
            vec![
                data_input("y", "Y", numeric_data_series_type())?,
                data_input("clusters", "Clusters", label_series()?)?,
                bounded_user_data_input("x", "X", numeric_data_series_type(), 1, None)?,
            ]
        } else if prediction {
            vec![data_input(
                "model",
                "Fitted model",
                fitted_regression_type()?,
            )?]
        } else if interval {
            vec![
                data_input("estimates", "Estimates", numeric_data_series_type())?,
                data_input(
                    "standard_errors",
                    "Standard errors",
                    numeric_data_series_type(),
                )?,
            ]
        } else {
            vec![
                data_input("y", "Y", numeric_data_series_type())?,
                data_input("groups", "Groups", label_series()?)?,
            ]
        };
        ports.push(data_output("result", "Result", report_type()?)?);
        if !cluster && !prediction {
            ports.push(if interval {
                fixed_numeric_table(
                    "intervals",
                    "Intervals",
                    &["index", "estimate", "standard_error", "lower", "upper"],
                )?
            } else {
                fixed_table(
                    "comparisons",
                    "Comparisons",
                    [
                        ("group_a", SemanticType::Numeric),
                        ("group_b", SemanticType::Numeric),
                        ("group_a_label", SemanticType::Text),
                        ("group_b_label", SemanticType::Text),
                        ("estimate", SemanticType::Numeric),
                        ("standard_error", SemanticType::Numeric),
                        ("degrees_of_freedom", SemanticType::Numeric),
                        ("statistic", SemanticType::Numeric),
                        ("p_value", SemanticType::Numeric),
                        ("adjusted_p_value", SemanticType::Numeric),
                        ("lower", SemanticType::Numeric),
                        ("upper", SemanticType::Numeric),
                    ],
                )?
            });
        }
        let mut parameters = if cluster {
            vec![toggle_parameter("constant", true)?]
        } else {
            vec![decimal_parameter("confidence_level", "0.95")?]
        };
        if prediction {
            parameters.extend([
                choice_parameter("evaluation", "average", &["average", "at_means"])?,
                parameter(
                    "at",
                    concrete("core.text")?,
                    ParameterEditorSpec::Text { multiline: false },
                    DataValue::String("".into()),
                    vec![],
                )?,
            ]);
        } else if cluster {
        } else if interval {
            parameters.push(decimal_parameter("degrees_of_freedom", "0")?);
        } else {
            parameters.extend([
                toggle_parameter("equal_variances", true)?,
                choice_parameter("adjustment", "holm", &["none", "bonferroni", "holm"])?,
            ]);
        }
        node_scoped_keys(&id, &mut parameters)?;
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: None,
                    category_id: sid(
                        if prediction {
                            "statistics.postestimation"
                        } else {
                            "statistics.inference"
                        },
                        NodeCategoryId::new,
                    )?,
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
