//! Executable independent-sample hypothesis tests.
use super::*;
use crate::builtin::assembled_decimal;
use yss_data_contract::DataValue;

const SPECS: &[(&str, &str)] = &[
    ("yssbi.statistics.test.t.one_sample", "one"),
    ("yssbi.statistics.test.t.independent", "independent"),
    ("yssbi.statistics.test.t.paired", "paired"),
    ("yssbi.statistics.test.t.summary_input", "summary"),
    ("yssbi.statistics.test.z.mean", "zmean"),
    ("yssbi.statistics.test.z.proportion", "oneprop"),
    ("yssbi.statistics.test.binomial", "binomial"),
    ("yssbi.statistics.test.proportion.two", "twoprop"),
    ("yssbi.statistics.test.chisquare.crosstab", "crosstab"),
    ("yssbi.statistics.test.chisquare.general", "pearson_table"),
    ("yssbi.statistics.test.chisquare.goodness_of_fit", "gof"),
    ("yssbi.statistics.test.fisher_exact", "fisher"),
    ("yssbi.statistics.test.mcnemar", "mcnemar"),
    ("yssbi.statistics.test.cmh", "cmh"),
    ("yssbi.statistics.test.proportion.multiple", "mult_prop"),
    ("yssbi.statistics.test.poisson", "poisson"),
    ("yssbi.statistics.test.equivalence", "equivalence"),
    (
        "yssbi.statistics.test.nonparametric.family",
        "nonparam_family",
    ),
    ("yssbi.statistics.test.wilcoxon.one_sample", "wilcoxon_one"),
    ("yssbi.statistics.test.wilcoxon.paired", "wilcoxon_paired"),
    ("yssbi.statistics.test.friedman", "friedman"),
    ("yssbi.statistics.test.runs", "runs"),
    ("yssbi.statistics.test.cochran_q", "cochran_q"),
    ("yssbi.statistics.test.mood_median", "mood_median"),
    ("yssbi.statistics.test.mann_kendall", "mann_kendall"),
    ("yssbi.statistics.test.mann_whitney", "mann_whitney"),
    ("yssbi.statistics.test.kruskal_wallis", "kruskal"),
    ("yssbi.statistics.test.levene", "levene"),
    ("yssbi.statistics.test.brown_forsythe", "brown_forsythe"),
    ("yssbi.statistics.test.bartlett", "bartlett"),
];

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(id, mode) in SPECS {
        let parameters = match mode {
            "one" => vec![
                decimal_parameter(id, "null_mean", "0")?,
                alternative_parameter()?,
            ],
            "independent" => vec![
                alternative_parameter()?,
                toggle_parameter("equal_variance", false)?,
            ],
            "paired" => vec![alternative_parameter()?],
            "summary" => vec![
                choice_parameter(
                    "design",
                    "independent",
                    &["one_sample", "independent", "paired"],
                )?,
                decimal_parameter(id, "null_value", "0")?,
                alternative_parameter()?,
                toggle_parameter("equal_variance", false)?,
            ],
            "zmean" => vec![
                decimal_parameter(id, "null_mean", "0")?,
                positive_decimal_parameter(id, "population_sd", "1")?,
                alternative_parameter()?,
            ],
            "oneprop" | "binomial" => vec![
                decimal_parameter(id, "null_probability", "0.5")?,
                alternative_parameter()?,
            ],
            "twoprop" => vec![
                decimal_parameter(id, "null_difference", "0")?,
                alternative_parameter()?,
            ],
            "pearson_table" => vec![
                minimum_integer_parameter("rows", 2, 2)?,
                minimum_integer_parameter("columns", 2, 2)?,
            ],
            "poisson" => vec![
                decimal_parameter(id, "null_rate", "1")?,
                alternative_parameter()?,
            ],
            "equivalence" => vec![
                decimal_parameter(id, "lower_bound", "-0.5")?,
                decimal_parameter(id, "upper_bound", "0.5")?,
            ],
            "wilcoxon_one" => vec![
                decimal_parameter(id, "null_median", "0")?,
                alternative_parameter()?,
            ],
            "wilcoxon_paired" | "mann_whitney" | "mann_kendall" => vec![alternative_parameter()?],
            "nonparam_family" => vec![choice_parameter(
                "method",
                "mann_whitney",
                &["mann_whitney", "kruskal_wallis", "mood_median"],
            )?],
            "crosstab" | "gof" | "fisher" | "mcnemar" | "cmh" | "mult_prop" | "friedman"
            | "runs" | "cochran_q" | "mood_median" | "kruskal" | "levene" | "brown_forsythe"
            | "bartlett" => vec![],
            _ => {
                return Err(BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
                    context: "classical hypothesis test",
                    value: mode.into(),
                });
            }
        };
        let mut ports = match mode {
            "one" => vec![data_input(
                "series",
                "Observations",
                numeric_data_series_type(),
            )?],
            "independent" | "mann_whitney" => vec![
                data_input("group1", "Group 1", numeric_data_series_type())?,
                data_input("group2", "Group 2", numeric_data_series_type())?,
            ],
            "paired" => vec![
                data_input("before", "Before", numeric_data_series_type())?,
                data_input("after", "After", numeric_data_series_type())?,
            ],
            "summary" => {
                vec![data_input(
                    "series",
                    "Summary values by design",
                    numeric_data_series_type(),
                )?]
            }
            "zmean" | "oneprop" | "binomial" => {
                vec![data_input(
                    "series",
                    "Observations",
                    numeric_data_series_type(),
                )?]
            }
            "twoprop" => vec![
                data_input("group1", "Group 1", numeric_data_series_type())?,
                data_input("group2", "Group 2", numeric_data_series_type())?,
            ],
            "crosstab" | "fisher" => vec![
                data_input("row", "Row classification", category_series_type()?)?,
                data_input("column", "Column classification", category_series_type()?)?,
            ],
            "pearson_table" => vec![data_input(
                "counts",
                "Observed cell counts",
                numeric_data_series_type(),
            )?],
            "gof" => vec![
                data_input("observed", "Observed counts", numeric_data_series_type())?,
                data_input("expected", "Expected counts", numeric_data_series_type())?,
            ],
            "mcnemar" => vec![
                data_input("before", "Before (0/1)", category_series_type()?)?,
                data_input("after", "After (0/1)", category_series_type()?)?,
            ],
            "cmh" => vec![
                data_input("exposed", "Exposure (0/1)", category_series_type()?)?,
                data_input("outcome", "Outcome (0/1)", category_series_type()?)?,
                data_input("strata", "Stratum", category_series_type()?)?,
            ],
            "mult_prop" => vec![data_input(
                "successes_and_trials",
                "Successes and trial counts",
                numeric_data_series_type(),
            )?],
            "poisson" | "equivalence" => vec![data_input(
                "series",
                "Counts or measurements",
                numeric_data_series_type(),
            )?],
            "nonparam_family" | "mood_median" | "kruskal" | "levene" | "brown_forsythe"
            | "bartlett" => vec![bounded_user_data_input(
                "groups",
                "Independent samples",
                numeric_data_series_type(),
                2,
                None,
            )?],
            "wilcoxon_one" | "runs" | "mann_kendall" => {
                vec![data_input(
                    "series",
                    "Observations",
                    numeric_data_series_type(),
                )?]
            }
            "wilcoxon_paired" => vec![
                data_input("before", "Before", numeric_data_series_type())?,
                data_input("after", "After", numeric_data_series_type())?,
            ],
            "friedman" | "cochran_q" => vec![bounded_user_data_input(
                "conditions",
                "Repeated conditions",
                numeric_data_series_type(),
                3,
                None,
            )?],
            _ => unreachable!(),
        };
        ports.push(data_output("result", "Result", report_type()?)?);
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id, NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(id, "title")?,
                    aliases_key: Some(node_key(id, "aliases")?),
                    category_id: sid("statistics.tests", NodeCategoryId::new)?,
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
    }
    Ok(())
}

fn decimal_parameter(
    node: &'static str,
    key: &'static str,
    default: &'static str,
) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.numeric")?,
        ParameterEditorSpec::Number,
        DataValue::Decimal(assembled_decimal(node, default)?),
        vec![],
    )
}

fn positive_decimal_parameter(
    node: &'static str,
    key: &'static str,
    default: &'static str,
) -> Result<Parameter, BuiltinAssemblyError> {
    let mut parameter = decimal_parameter(node, key, default)?;
    parameter.constraints.push(ParameterConstraint::Positive);
    Ok(parameter)
}

fn category_series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    let members = [
        "core.numeric",
        "core.categorical",
        "core.ordinal",
        "core.binary",
    ]
    .into_iter()
    .map(|id| concrete(id).map(data_series_type))
    .collect::<Result<Vec<_>, _>>()?;
    normalize_type_expr(TypeExpr::Union(members)).map_err(|error| {
        BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
            context: "categorical test input",
            value: error.to_string().into(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_node_protocol::{ParameterIssueKind, ParameterValues, validate_parameter_values};

    #[test]
    fn pearson_dimensions_accept_resource_admitted_tables_above_one_thousand() {
        let system = crate::build_builtin_node_system().unwrap();
        let protocol = system
            .registry
            .protocol(&"yssbi.statistics.test.chisquare.general".parse().unwrap())
            .unwrap();
        let rows = ParameterKey::new("rows").unwrap();
        let columns = ParameterKey::new("columns").unwrap();
        for (row_count, column_count) in [(2, 1001), (1001, 2)] {
            let values = ParameterValues::from([
                (rows.clone(), row_count.into()),
                (columns.clone(), column_count.into()),
            ]);
            let issues = validate_parameter_values(protocol, &values, system.registry.as_ref());
            assert!(
                issues.is_empty(),
                "resource-admitted {row_count} by {column_count} table rejected: {issues:?}"
            );
        }
        let values = ParameterValues::from([(rows.clone(), 1.into()), (columns, 2.into())]);
        assert!(
            validate_parameter_values(protocol, &values, system.registry.as_ref())
                .iter()
                .any(|issue| issue.key == rows
                    && matches!(issue.kind, ParameterIssueKind::Constraint))
        );
    }
}
