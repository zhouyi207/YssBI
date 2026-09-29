//! Executable independent-sample hypothesis tests.
use super::*;
use crate::builtin::assembled_decimal;
use crate::builtin::node_key_text;
use yss_data_contract::DataValue;

const SPECS: &[(&str, &str, &str, &[&str], &str)] = &[
    (
        "yssbi.statistics.test.t.one_sample",
        "One-sample t test",
        "单样本 t 检验",
        &["t.test one sample", "one sample mean"],
        "one",
    ),
    (
        "yssbi.statistics.test.t.independent",
        "Independent-samples t test",
        "独立样本 t 检验",
        &["welch t test", "student t test", "two sample mean"],
        "independent",
    ),
    (
        "yssbi.statistics.test.t.paired",
        "Paired t test",
        "配对样本 t 检验",
        &["dependent t test", "paired mean"],
        "paired",
    ),
    (
        "yssbi.statistics.test.t.summary_input",
        "Summary-statistics t test",
        "概要统计 t 检验",
        &["summary t test", "mean sd n"],
        "summary",
    ),
    (
        "yssbi.statistics.test.z.mean",
        "One-sample z test for a mean",
        "单样本均值 z 检验",
        &["known standard deviation", "z test mean"],
        "zmean",
    ),
    (
        "yssbi.statistics.test.z.proportion",
        "One-proportion z test",
        "单样本比例 z 检验",
        &["one proportion", "proportion test"],
        "oneprop",
    ),
    (
        "yssbi.statistics.test.binomial",
        "Exact binomial test",
        "精确二项检验",
        &["binomial test", "exact proportion test"],
        "binomial",
    ),
    (
        "yssbi.statistics.test.proportion.two",
        "Two-proportion z test",
        "两比例 z 检验",
        &["two proportion", "difference in proportions"],
        "twoprop",
    ),
    (
        "yssbi.statistics.test.chisquare.crosstab",
        "Chi-square test of independence",
        "列联表卡方独立性检验",
        &["crosstab", "chi square independence"],
        "crosstab",
    ),
    (
        "yssbi.statistics.test.chisquare.general",
        "Pearson chi-square contingency test",
        "Pearson 列联表卡方检验",
        &["pearson chi square", "contingency table"],
        "pearson_table",
    ),
    (
        "yssbi.statistics.test.chisquare.goodness_of_fit",
        "Chi-square goodness-of-fit test",
        "卡方拟合优度检验",
        &["chi square goodness of fit"],
        "gof",
    ),
    (
        "yssbi.statistics.test.fisher_exact",
        "Fisher exact test",
        "Fisher 精确检验",
        &["fisher exact 2x2"],
        "fisher",
    ),
    (
        "yssbi.statistics.test.mcnemar",
        "McNemar test",
        "McNemar 配对二分类检验",
        &["paired proportions"],
        "mcnemar",
    ),
    (
        "yssbi.statistics.test.cmh",
        "Cochran–Mantel–Haenszel test",
        "Cochran–Mantel–Haenszel 分层检验",
        &["cmh stratified test"],
        "cmh",
    ),
    (
        "yssbi.statistics.test.proportion.multiple",
        "Multiple-proportion homogeneity test",
        "多组比例齐性检验",
        &["multiple proportions"],
        "mult_prop",
    ),
    (
        "yssbi.statistics.test.poisson",
        "Poisson rate test",
        "Poisson 率检验",
        &["poisson count test"],
        "poisson",
    ),
    (
        "yssbi.statistics.test.equivalence",
        "One-sample equivalence test (TOST)",
        "单样本等价检验（TOST）",
        &["two one-sided tests", "tost"],
        "equivalence",
    ),
    (
        "yssbi.statistics.test.nonparametric.family",
        "Nonparametric group-test selector",
        "非参数组间检验选择器",
        &["nonparametric tests", "rank tests"],
        "nonparam_family",
    ),
    (
        "yssbi.statistics.test.wilcoxon.one_sample",
        "One-sample Wilcoxon signed-rank test",
        "单样本 Wilcoxon 符号秩检验",
        &["signed rank one sample"],
        "wilcoxon_one",
    ),
    (
        "yssbi.statistics.test.wilcoxon.paired",
        "Paired Wilcoxon signed-rank test",
        "配对 Wilcoxon 符号秩检验",
        &["signed rank paired"],
        "wilcoxon_paired",
    ),
    (
        "yssbi.statistics.test.friedman",
        "Friedman repeated-measures test",
        "Friedman 重复测量检验",
        &["friedman test"],
        "friedman",
    ),
    (
        "yssbi.statistics.test.runs",
        "Runs test for randomness",
        "游程随机性检验",
        &["runs test randomness"],
        "runs",
    ),
    (
        "yssbi.statistics.test.cochran_q",
        "Cochran Q repeated-proportions test",
        "Cochran Q 重复比例检验",
        &["cochran q"],
        "cochran_q",
    ),
    (
        "yssbi.statistics.test.mood_median",
        "Mood median test",
        "Mood 中位数检验",
        &["mood median"],
        "mood_median",
    ),
    (
        "yssbi.statistics.test.mann_kendall",
        "Mann–Kendall trend test",
        "Mann–Kendall 趋势检验",
        &["monotonic trend"],
        "mann_kendall",
    ),
    (
        "yssbi.statistics.test.mann_whitney",
        "Mann–Whitney U test",
        "Mann–Whitney U 检验",
        &["wilcoxon rank sum"],
        "mann_whitney",
    ),
    (
        "yssbi.statistics.test.kruskal_wallis",
        "Kruskal–Wallis test",
        "Kruskal–Wallis 检验",
        &["rank one way anova"],
        "kruskal",
    ),
    (
        "yssbi.statistics.test.levene",
        "Levene test",
        "Levene 方差齐性检验",
        &["variance homogeneity"],
        "levene",
    ),
    (
        "yssbi.statistics.test.brown_forsythe",
        "Brown–Forsythe test",
        "Brown–Forsythe 方差齐性检验",
        &["median-centered levene"],
        "brown_forsythe",
    ),
    (
        "yssbi.statistics.test.bartlett",
        "Bartlett test",
        "Bartlett 方差齐性检验",
        &["bartlett variance"],
        "bartlett",
    ),
];

pub(super) fn implemented(id: &str) -> bool {
    SPECS.iter().any(|spec| spec.0 == id)
}

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    add_parameter_messages(&mut fragment.messages);
    for &(id, en, zh, aliases, mode) in SPECS {
        let parameters = match mode {
            "one" => vec![
                decimal_parameter(id, "null_mean", "0")?,
                alternative_parameter(id)?,
            ],
            "independent" => vec![
                choice_parameter(
                    "alternative",
                    "two_sided",
                    &["two_sided", "greater", "less"],
                )?,
                toggle_parameter("equal_variance", false)?,
            ],
            "paired" => vec![choice_parameter(
                "alternative",
                "two_sided",
                &["two_sided", "greater", "less"],
            )?],
            "summary" => vec![
                choice_parameter(
                    "design",
                    "independent",
                    &["one_sample", "independent", "paired"],
                )?,
                decimal_parameter(id, "null_value", "0")?,
                choice_parameter(
                    "alternative",
                    "two_sided",
                    &["two_sided", "greater", "less"],
                )?,
                toggle_parameter("equal_variance", false)?,
            ],
            "zmean" => vec![
                decimal_parameter(id, "null_mean", "0")?,
                positive_decimal_parameter(id, "population_sd", "1")?,
                choice_parameter(
                    "alternative",
                    "two_sided",
                    &["two_sided", "greater", "less"],
                )?,
            ],
            "oneprop" | "binomial" => vec![
                unit_interval_parameter(id, "null_probability", "0.5")?,
                choice_parameter(
                    "alternative",
                    "two_sided",
                    &["two_sided", "greater", "less"],
                )?,
            ],
            "twoprop" => vec![
                decimal_parameter(id, "null_difference", "0")?,
                choice_parameter(
                    "alternative",
                    "two_sided",
                    &["two_sided", "greater", "less"],
                )?,
            ],
            "pearson_table" => vec![
                bounded_integer_parameter("rows", 2, 2, 1000)?,
                bounded_integer_parameter("columns", 2, 2, 1000)?,
            ],
            "poisson" => vec![
                decimal_parameter(id, "null_rate", "1")?,
                choice_parameter(
                    "alternative",
                    "two_sided",
                    &["two_sided", "greater", "less"],
                )?,
            ],
            "equivalence" => vec![
                decimal_parameter(id, "lower_bound", "-0.5")?,
                decimal_parameter(id, "upper_bound", "0.5")?,
            ],
            "wilcoxon_one" => vec![
                decimal_parameter(id, "null_median", "0")?,
                choice_parameter(
                    "alternative",
                    "two_sided",
                    &["two_sided", "greater", "less"],
                )?,
            ],
            "wilcoxon_paired" | "mann_whitney" | "mann_kendall" => vec![choice_parameter(
                "alternative",
                "two_sided",
                &["two_sided", "greater", "less"],
            )?],
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
            "one" => vec![data_input("series", "Observations", series_type()?)?],
            "independent" | "mann_whitney" => vec![
                data_input("group1", "Group 1", series_type()?)?,
                data_input("group2", "Group 2", series_type()?)?,
            ],
            "paired" => vec![
                data_input("before", "Before", series_type()?)?,
                data_input("after", "After", series_type()?)?,
            ],
            "summary" => {
                vec![data_input(
                    "series",
                    "Summary values by design",
                    series_type()?,
                )?]
            }
            "zmean" | "oneprop" | "binomial" => {
                vec![data_input("series", "Observations", series_type()?)?]
            }
            "twoprop" => vec![
                data_input("group1", "Group 1", series_type()?)?,
                data_input("group2", "Group 2", series_type()?)?,
            ],
            "crosstab" | "fisher" => vec![
                data_input("row", "Row classification", category_series_type()?)?,
                data_input("column", "Column classification", category_series_type()?)?,
            ],
            "pearson_table" => vec![data_input(
                "counts",
                "Observed cell counts",
                series_type()?,
            )?],
            "gof" => vec![
                data_input("observed", "Observed counts", series_type()?)?,
                data_input("expected", "Expected counts", series_type()?)?,
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
                series_type()?,
            )?],
            "poisson" | "equivalence" => vec![data_input(
                "series",
                "Counts or measurements",
                series_type()?,
            )?],
            "nonparam_family" | "mood_median" | "kruskal" | "levene" | "brown_forsythe"
            | "bartlett" => vec![bounded_user_data_input(
                "groups",
                "Independent samples",
                series_type()?,
                2,
                Some(16),
            )?],
            "wilcoxon_one" | "runs" | "mann_kendall" => {
                vec![data_input("series", "Observations", series_type()?)?]
            }
            "wilcoxon_paired" => vec![
                data_input("before", "Before", series_type()?)?,
                data_input("after", "After", series_type()?)?,
            ],
            "friedman" | "cochran_q" => vec![bounded_user_data_input(
                "conditions",
                "Repeated conditions",
                series_type()?,
                3,
                Some(16),
            )?],
            _ => unreachable!(),
        };
        ports.push(data_output("result", "Result", report_type()?)?);
        ports.push(data_output("report", "Report", report_type()?)?);
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id, NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(id, "title")?,
                    documentation_key: Some(node_key(id, "documentation")?),
                    aliases_key: Some(node_key(id, "aliases")?),
                    category_id: sid("statistics.tests", NodeCategoryId::new)?,
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
        for (locale, title, description) in [
            (
                "en-US",
                en,
                "Test a statistical hypothesis using observations or summary data. Outputs the result and report; see the help for required inputs and assumptions.",
            ),
            (
                "zh-CN",
                zh,
                "使用观测值或概要数据进行假设检验，输出检验结果和报告；具体输入要求与适用假设见帮助文档。",
            ),
        ] {
            fragment
                .messages
                .push((locale, node_key_text(id, "title"), Text(title)));
            fragment.messages.push((
                locale,
                node_key_text(id, "documentation"),
                Text(description),
            ));
            fragment
                .messages
                .push((locale, node_key_text(id, "aliases"), Aliases(aliases)));
        }
    }
    Ok(())
}

fn add_parameter_messages(out: &mut Vec<(&'static str, String, Message)>) {
    for (key, en, zh, en_description, zh_description) in [
        (
            "alternative",
            "Alternative hypothesis",
            "备择假设",
            "Choose a two-sided, greater-than or less-than alternative.",
            "选择双侧、大于或小于方向的备择假设。",
        ),
        (
            "equal_variance",
            "Assume equal variances",
            "假定方差相等",
            "Use pooled variance for independent samples; otherwise use Welch's test.",
            "独立样本使用合并方差；关闭时使用 Welch 检验。",
        ),
        (
            "design",
            "Study design",
            "研究设计",
            "Interpret summary values as one sample, independent samples or paired differences.",
            "将概要统计量解释为单样本、独立样本或配对差值。",
        ),
        (
            "null_mean",
            "Hypothesized mean",
            "原假设均值",
            "Population mean specified by the null hypothesis.",
            "原假设指定的总体均值。",
        ),
        (
            "null_value",
            "Hypothesized mean or difference",
            "原假设均值或差值",
            "Mean or mean difference under the null hypothesis, according to the study design.",
            "研究设计所对应的原假设均值或均值差。",
        ),
        (
            "population_sd",
            "Known population standard deviation",
            "已知总体标准差",
            "Positive, known population standard deviation for the z test.",
            "z 检验使用的已知总体标准差，必须为正。",
        ),
        (
            "null_probability",
            "Hypothesized probability",
            "原假设概率",
            "Success probability under the null hypothesis.",
            "原假设指定的成功概率。",
        ),
        (
            "null_difference",
            "Hypothesized proportion difference",
            "原假设比例差",
            "Difference between the two population proportions under the null hypothesis.",
            "原假设指定的两个总体比例之差。",
        ),
        (
            "rows",
            "Contingency table rows",
            "列联表行数",
            "Number of rows in the contingency table of observed counts.",
            "观测频数列联表的行数。",
        ),
        (
            "columns",
            "Contingency table columns",
            "列联表列数",
            "Number of columns in the contingency table of observed counts.",
            "观测频数列联表的列数。",
        ),
        (
            "null_rate",
            "Hypothesized Poisson rate",
            "原假设 Poisson 率",
            "Expected count per observation under the null hypothesis.",
            "原假设指定的每次观测期望计数。",
        ),
        (
            "lower_bound",
            "Lower equivalence bound",
            "等价区间下界",
            "Lower mean bound for the equivalence test, below the upper bound.",
            "等价检验的均值下界，必须小于上界。",
        ),
        (
            "upper_bound",
            "Upper equivalence bound",
            "等价区间上界",
            "Upper mean bound for the equivalence test, above the lower bound.",
            "等价检验的均值上界，必须大于下界。",
        ),
        (
            "null_median",
            "Hypothesized median",
            "原假设中位数",
            "Reference location for the one-sample signed-rank test.",
            "单样本符号秩检验使用的参照位置。",
        ),
    ] {
        for (locale, title, description) in
            [("en-US", en, en_description), ("zh-CN", zh, zh_description)]
        {
            out.extend([
                (
                    locale,
                    format!("parameters.statistics.{key}.title"),
                    Text(title),
                ),
                (
                    locale,
                    format!("parameters.statistics.{key}.description"),
                    Text(description),
                ),
            ]);
        }
    }
}

fn alternative_parameter(_id: &str) -> Result<Parameter, BuiltinAssemblyError> {
    choice_parameter(
        "alternative",
        "two_sided",
        &["two_sided", "greater", "less"],
    )
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

fn unit_interval_parameter(
    node: &'static str,
    key: &'static str,
    default: &'static str,
) -> Result<Parameter, BuiltinAssemblyError> {
    decimal_parameter(node, key, default)
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
