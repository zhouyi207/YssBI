//! Executable correlation and inter-rater agreement nodes.
use super::*;

const SPECS: &[(&str, &str, &str, &str, &str, &str)] = &[
    (
        "yssbi.statistics.association.pearson",
        "Pearson correlation",
        "Pearson 相关",
        "pearson",
        "Compute paired Pearson correlation, its t test and Fisher confidence interval.",
        "计算配对数值的 Pearson 相关系数、t 检验和 Fisher 置信区间。",
    ),
    (
        "yssbi.statistics.association.partial",
        "Partial correlation",
        "偏相关",
        "partial",
        "Compute Pearson correlation after controlling for aligned numerical covariates.",
        "控制对齐的数值协变量后计算 Pearson 偏相关及其推断。",
    ),
    (
        "yssbi.statistics.association.spearman",
        "Spearman rank correlation",
        "Spearman 秩相关",
        "spearman",
        "Compute Spearman rho with average ranks for ties and exact-permutation or approximate inference.",
        "使用并列平均秩计算 Spearman rho，支持精确置换或渐近检验。",
    ),
    (
        "yssbi.statistics.association.kendall",
        "Kendall rank correlation",
        "Kendall 秩相关",
        "kendall",
        "Compute Kendall tau-b with tie corrections and exact-permutation or approximate inference.",
        "计算处理并列值的 Kendall tau-b，支持精确置换或渐近检验。",
    ),
    (
        "yssbi.statistics.test.kappa",
        "Kappa agreement",
        "Kappa 一致性",
        "kappa",
        "Choose Cohen or Fleiss kappa; Cohen supports linear and quadratic weighting for ordered categories.",
        "选择 Cohen 或 Fleiss Kappa；Cohen 支持有序类别的线性、二次加权。",
    ),
    (
        "yssbi.statistics.association.icc",
        "Intraclass correlation",
        "ICC 组内相关系数",
        "icc",
        "Choose ICC1/2/3 and single/average measurement definitions with F inference.",
        "选择 ICC1/2/3 的单次或平均测量定义，输出 F 检验和置信区间。",
    ),
    (
        "yssbi.statistics.association.bland_altman",
        "Bland–Altman agreement",
        "Bland–Altman 一致性",
        "bland_altman",
        "Compute paired bias, limits of agreement, confidence intervals and bounded observation points.",
        "计算配对方法的偏倚、一致性界限、置信区间及有界观测点。",
    ),
    (
        "yssbi.statistics.test.kendall_w",
        "Kendall concordance W",
        "Kendall 协调系数 W",
        "kendall_w",
        "Compute Kendall W across aligned raters, including tie correction and chi-square inference.",
        "计算对齐评定者的 Kendall W，包含并列修正和卡方近似检验。",
    ),
    (
        "yssbi.statistics.association.ridit",
        "Ridit analysis",
        "Ridit 分析",
        "ridit",
        "Compare an ordered sample to a reference distribution, with category ridits and rank inference.",
        "将有序样本与参考分布比较，输出类别 Ridit、平均 Ridit 和秩检验。",
    ),
    (
        "yssbi.statistics.association.rwg",
        "Within-group agreement rwg",
        "组内一致性 rwg",
        "rwg",
        "Compute single-item and multi-item within-group agreement against a stated null variance.",
        "按明确的零假设方差计算单条目和多条目组内一致性。",
    ),
];

fn aliases(method: &str) -> &'static [&'static str] {
    match method {
        "pearson" => &["association.pearson", "Pearson", "相关"],
        "partial" => &["association.partial", "partial correlation", "偏相关"],
        "spearman" => &["association.spearman", "Spearman", "秩相关"],
        "kendall" => &["association.kendall", "Kendall tau-b", "秩相关"],
        "kappa" => &["test.kappa", "Cohen Kappa", "Fleiss Kappa"],
        "icc" => &["association.icc", "ICC", "组内相关系数"],
        "bland_altman" => &["association.bland_altman", "Bland-Altman", "一致性界限"],
        "kendall_w" => &["test.kendall_w", "Kendall W", "协调系数"],
        "ridit" => &["association.ridit", "Ridit", "Ridit分析"],
        "rwg" => &["association.rwg", "rwg", "组内一致性"],
        _ => unreachable!(),
    }
}
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
fn category_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    union_series(&[
        "core.numeric",
        "core.categorical",
        "core.ordinal",
        "core.binary",
        "core.text",
        "core.identifier",
    ])
}
fn condition(key: &str, value: &str) -> Result<ParameterCondition, BuiltinAssemblyError> {
    Ok(ParameterCondition {
        key: sid(key, ParameterKey::new)?,
        values: vec![DataValue::String(value.into())].into_boxed_slice(),
    })
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
fn alternative() -> Result<Parameter, BuiltinAssemblyError> {
    choice_parameter(
        "alternative",
        "two_sided",
        &["two_sided", "greater", "less"],
    )
}
fn parameters(method: &str) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    Ok(match method {
        "pearson" | "partial" => vec![alternative()?, confidence()?],
        "spearman" | "kendall" => vec![
            alternative()?,
            choice_parameter(
                "p_value_method",
                "auto",
                &["auto", "permutation_exact", "asymptotic"],
            )?,
        ],
        "kappa" => vec![
            choice_parameter("kappa_method", "cohen", &["cohen", "fleiss"])?,
            choice_parameter("kappa_weighting", "none", &["none", "linear", "quadratic"])?
                .when(condition("kappa_method", "cohen")?),
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
            alternative()?,
            toggle_parameter("continuity_correction", false)?,
        ],
        "rwg" => vec![
            choice_parameter(
                "null_distribution",
                "uniform",
                &["uniform", "specified_variance"],
            )?,
            bounded_integer_parameter("scale_points", 5, 2, 10000)?
                .when(condition("null_distribution", "uniform")?),
            tolerance_parameter_for("expected_variance", "2")?
                .when(condition("null_distribution", "specified_variance")?),
        ],
        _ => unreachable!(),
    })
}

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(id, en, zh, method, en_help, zh_help) in SPECS {
        let mut ports = match method {
            "pearson" | "partial" | "bland_altman" => vec![
                data_input("x", "X / method A", series_type()?)?,
                data_input("y", "Y / method B", series_type()?)?,
            ],
            "spearman" | "kendall" => vec![
                data_input("x", "X", rank_type()?)?,
                data_input("y", "Y", rank_type()?)?,
            ],
            "kappa" | "icc" | "kendall_w" => vec![bounded_user_data_input(
                "ratings",
                "Rater",
                match method {
                    "kappa" => category_type()?,
                    "kendall_w" => rank_type()?,
                    _ => series_type()?,
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
                series_type()?,
                1,
                None,
            )?],
            _ => unreachable!(),
        };
        if method == "partial" {
            ports.push(bounded_user_data_input(
                "controls",
                "Control variable",
                series_type()?,
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
                    documentation_key: Some(node_key(id, "documentation")?),
                    aliases_key: Some(node_key(id, "aliases")?),
                    category_id: sid("statistics.association", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(id, ports, vec![], vec![])?,
                parameters: assembled_parameters(id, parameters(method)?)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: execution(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            id,
        ));
        for (locale, title, help) in [("en-US", en, en_help), ("zh-CN", zh, zh_help)] {
            fragment.messages.extend([
                (locale, node_key_text(id, "title"), Text(title)),
                (locale, node_key_text(id, "documentation"), Text(help)),
                (
                    locale,
                    node_key_text(id, "aliases"),
                    Aliases(aliases(method)),
                ),
            ]);
        }
    }
    for (key, en, zh, en_help, zh_help) in [
        (
            "confidence_level",
            "Confidence level",
            "置信水平",
            "A value strictly between 0 and 1; default 0.95.",
            "必须在 0 与 1 之间，默认 0.95。",
        ),
        (
            "p_value_method",
            "P-value method",
            "p 值方法",
            "Auto uses exact permutations for at most 9 observations; otherwise asymptotic inference.",
            "auto 在不超过 9 个观测时使用精确置换，否则使用渐近检验。",
        ),
        (
            "kappa_method",
            "Kappa definition",
            "Kappa 定义",
            "Cohen requires exactly two raters; Fleiss pools the category margins across raters.",
            "Cohen 必须恰好两位评定者；Fleiss 使用各评定者合并的类别边际分布。",
        ),
        (
            "kappa_weighting",
            "Category weighting",
            "类别权重",
            "Linear/quadratic weighting requires numerical scores or identical explicit ordinal category orders.",
            "线性/二次加权要求数值评分或相同的显式有序类别表。",
        ),
        (
            "icc_type",
            "ICC definition",
            "ICC 定义",
            "ICC1: one-way; ICC2: two-way absolute agreement; ICC3: two-way consistency. k means average measurement.",
            "ICC1：单向；ICC2：双向绝对一致；ICC3：双向一致性。k 表示平均测量。",
        ),
        (
            "coverage",
            "Agreement-limit coverage",
            "一致性界限覆盖率",
            "The central normal coverage used for limits of agreement, strictly between 0 and 1.",
            "一致性界限采用的正态中央覆盖率，必须在 0 与 1 之间。",
        ),
        (
            "continuity_correction",
            "Continuity correction",
            "连续性修正",
            "Apply a half-unit continuity correction to the rank test.",
            "对秩检验应用半单位连续性修正。",
        ),
        (
            "null_distribution",
            "Null distribution",
            "零假设分布",
            "Use equally likely integer response options or specify their expected variance.",
            "使用等概率的整数评分选项，或指定零假设的期望方差。",
        ),
        (
            "scale_points",
            "Response options",
            "评分选项数",
            "Uniform ratings use integer scores from 1 through this value; default 5.",
            "均匀评分采用从 1 到该值的整数分数，默认 5。",
        ),
        (
            "expected_variance",
            "Expected null variance",
            "零假设期望方差",
            "A finite positive variance on the same scale as the ratings.",
            "与评分同一尺度上的有限正方差。",
        ),
    ] {
        for (locale, title, help) in [("en-US", en, en_help), ("zh-CN", zh, zh_help)] {
            fragment.messages.extend([
                (
                    locale,
                    format!("parameters.statistics.{key}.title"),
                    Text(title),
                ),
                (
                    locale,
                    format!("parameters.statistics.{key}.description"),
                    Text(help),
                ),
            ]);
        }
    }
    Ok(())
}
