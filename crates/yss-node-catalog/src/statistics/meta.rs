//! Study summaries, inverse-variance analyses and meta plots.
use super::*;
mod interface;
mod parameters;

const NODES: &[(&str, &str, &str)] = &[
    ("meta.continuous", "Continuous effect sizes", "连续型效应量"),
    ("meta.binary", "Binary effect sizes", "二分类效应量"),
    (
        "meta.single_proportion",
        "Single-proportion effect sizes",
        "单个率效应量",
    ),
    ("meta.mean", "Mean effect sizes", "平均值效应量"),
    (
        "meta.correlation",
        "Correlation effect sizes",
        "相关系数效应量",
    ),
    (
        "meta.or_hr",
        "Reported OR / HR effect sizes",
        "OR / HR 效应量",
    ),
    ("meta.combine_p", "Combine p-values", "P 值合并"),
    (
        "meta.inverse_variance",
        "Inverse-variance meta-analysis",
        "一般倒方差 Meta 分析",
    ),
    (
        "meta.fixed_effect",
        "Fixed-effect meta-analysis",
        "固定效应 Meta 分析",
    ),
    (
        "meta.random_effect",
        "Random-effects meta-analysis",
        "随机效应 Meta 分析",
    ),
    (
        "meta.cochran_q",
        "Cochran Q heterogeneity test",
        "Cochran Q 异质性检验",
    ),
    (
        "meta.i_squared",
        "I-squared heterogeneity",
        "I² 异质性统计量",
    ),
    (
        "meta.tau_squared",
        "Between-study variance",
        "τ² 研究间方差",
    ),
    ("meta.regression", "Meta-regression", "Meta 回归"),
    ("meta.egger", "Egger asymmetry test", "Egger 检验"),
    ("meta.begg", "Begg rank-correlation test", "Begg 检验"),
    (
        "meta.leave_one_out",
        "Leave-one-study-out analysis",
        "逐研究剔除分析",
    ),
    (
        "meta.sensitivity",
        "Meta-analysis sensitivity",
        "Meta 敏感性分析",
    ),
    ("plot.forest", "Forest plot", "森林图"),
    ("plot.funnel", "Funnel plot", "漏斗图"),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(suffix, en, zh) in NODES {
        let id = format!("yssbi.statistics.{suffix}");
        let method = suffix.rsplit('.').next().unwrap();
        let parameters = parameters::build(&id, method, fragment)?;
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
                    aliases_key: None,
                    category_id: sid("statistics.meta", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, interface::ports(method)?, vec![], vec![])?,
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
