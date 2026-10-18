//! Prospective scalar power interfaces with model-specific units and assumptions.
use super::*;
mod parameters;
const METHODS: &[(&str, &str, &str)] = &[
    (
        "principles",
        "Power principles (normal mean planning)",
        "功效原理（正态均值规划）",
    ),
    (
        "mean_difference",
        "Mean difference power (t test)",
        "均值差功效（t 检验）",
    ),
    ("paired", "Paired t-test power", "配对 t 检验功效"),
    (
        "variance",
        "Normal population variance power",
        "正态总体方差功效",
    ),
    (
        "proportion",
        "One-proportion power (normal approximation)",
        "单比例功效（正态近似）",
    ),
    (
        "proportion_difference",
        "Two-proportion power (normal approximation)",
        "两比例差功效（正态近似）",
    ),
    (
        "correlation",
        "Correlation power (Fisher z)",
        "相关性功效（Fisher z）",
    ),
    (
        "anova",
        "Balanced one-way ANOVA power",
        "平衡单因素 ANOVA 功效",
    ),
    (
        "linear_regression",
        "Overall linear regression power",
        "线性回归整体检验功效",
    ),
    (
        "generalized_model",
        "GLM power (Poisson rate ratio)",
        "广义模型功效（Poisson 发生率比）",
    ),
    (
        "logistic",
        "Logistic power (binary predictor)",
        "Logistic 功效（二元预测变量）",
    ),
    (
        "cox",
        "Cox power (Schoenfeld approximation)",
        "Cox 功效（Schoenfeld 近似）",
    ),
    (
        "logrank",
        "Log-rank power (Schoenfeld approximation)",
        "Log-rank 功效（Schoenfeld 近似）",
    ),
    (
        "cluster_randomized",
        "Cluster randomized power (continuous outcome)",
        "整群随机功效（连续结果）",
    ),
    (
        "noninferiority",
        "Mean noninferiority power (known variance)",
        "均值非劣效功效（已知方差）",
    ),
    (
        "equivalence",
        "Mean equivalence power (known variance)",
        "均值等效功效（已知方差）",
    ),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, en, zh) in METHODS {
        let id = format!("yssbi.statistics.power.{method}");
        let parameters = parameters::build(fragment, &id, method)?;
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
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
    }
    Ok(())
}
