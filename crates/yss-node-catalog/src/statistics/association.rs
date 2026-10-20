//! Executable correlation and inter-rater agreement nodes.
use super::*;

const SPECS: &[(&str, &str)] = &[
    ("yssbi.statistics.association.pearson", "pearson"),
    ("yssbi.statistics.association.partial", "partial"),
    ("yssbi.statistics.association.spearman", "spearman"),
    ("yssbi.statistics.association.kendall", "kendall"),
    ("yssbi.statistics.test.kappa", "kappa"),
    ("yssbi.statistics.association.icc", "icc"),
    ("yssbi.statistics.association.bland_altman", "bland_altman"),
    ("yssbi.statistics.test.kendall_w", "kendall_w"),
    ("yssbi.statistics.association.ridit", "ridit"),
    ("yssbi.statistics.association.rwg", "rwg"),
];

fn union_series(semantics: &[&'static str]) -> Result<TypeExpr, BuiltinAssemblyError> {
    let members = semantics
        .iter()
        .map(|id| concrete(id).map(data_series_type))
        .collect::<Result<Vec<_>, _>>()?;
    normalize_type_expr(TypeExpr::Union(members)).map_err(|error| {
        BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
            context: "association series input",
            value: error.to_string().into(),
        }
    })
}
fn rank_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    union_series(&["core.numeric", "core.ordinal"])
}
fn confidence() -> Result<Parameter, BuiltinAssemblyError> {
    tolerance_parameter_for("confidence_level", "0.95")
}
fn tolerance_parameter_for(
    key: &'static str,
    default: &'static str,
) -> Result<Parameter, BuiltinAssemblyError> {
    let mut field = decimal_parameter(key, default)?;
    field.constraints.push(ParameterConstraint::Positive);
    Ok(field)
}
fn parameters(method: &str) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    Ok(match method {
        "pearson" | "partial" => vec![alternative_parameter()?, confidence()?],
        "spearman" | "kendall" => vec![
            alternative_parameter()?,
            choice_parameter(
                "p_value_method",
                "auto",
                &["auto", "permutation_exact", "asymptotic"],
            )?,
        ],
        "kappa" => vec![
            choice_parameter("kappa_method", "cohen", &["cohen", "fleiss"])?,
            choice_parameter("kappa_weighting", "none", &["none", "linear", "quadratic"])?
                .when(choice_condition("kappa_method", "cohen")?),
            confidence()?,
        ],
        "icc" => vec![
            choice_parameter(
                "icc_type",
                "ICC2",
                &["ICC1", "ICC2", "ICC3", "ICC1k", "ICC2k", "ICC3k"],
            )?,
            confidence()?,
        ],
        "bland_altman" => vec![tolerance_parameter_for("coverage", "0.95")?, confidence()?],
        "kendall_w" => vec![],
        "ridit" => vec![
            alternative_parameter()?,
            toggle_parameter("continuity_correction", false)?,
        ],
        "rwg" => vec![
            choice_parameter(
                "null_distribution",
                "uniform",
                &["uniform", "specified_variance"],
            )?,
            bounded_integer_parameter("scale_points", 5, 2, 10000)?
                .when(choice_condition("null_distribution", "uniform")?),
            tolerance_parameter_for("expected_variance", "2")?
                .when(choice_condition("null_distribution", "specified_variance")?),
        ],
        _ => unreachable!(),
    })
}

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(id, method) in SPECS {
        let mut ports = match method {
            "pearson" | "partial" | "bland_altman" => vec![
                data_input("x", "X / method A", numeric_data_series_type())?,
                data_input("y", "Y / method B", numeric_data_series_type())?,
            ],
            "spearman" | "kendall" => vec![
                data_input("x", "X", rank_type()?)?,
                data_input("y", "Y", rank_type()?)?,
            ],
            "kappa" | "icc" | "kendall_w" => vec![bounded_user_data_input(
                "ratings",
                "Rater",
                match method {
                    "kappa" => label_series()?,
                    "kendall_w" => rank_type()?,
                    _ => numeric_data_series_type(),
                },
                2,
                None,
            )?],
            "ridit" => vec![
                data_input("sample", "Ordered sample", rank_type()?)?,
                data_input("reference", "Ordered reference", rank_type()?)?,
            ],
            "rwg" => vec![bounded_user_data_input(
                "items",
                "Scale item",
                numeric_data_series_type(),
                1,
                None,
            )?],
            _ => unreachable!(),
        };
        if method == "partial" {
            ports.push(bounded_user_data_input(
                "controls",
                "Control variable",
                numeric_data_series_type(),
                1,
                None,
            )?);
        }
        ports.push(data_output("result", "Result", report_type()?)?);
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id, NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(id, "title")?,
                    aliases_key: Some(node_key(id, "aliases")?),
                    category_id: sid("statistics.association", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(id, ports, vec![], vec![])?,
                parameters: assembled_parameters(id, parameters(method)?)?,
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
