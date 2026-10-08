//! Executable longitudinal nodes preserve the ten inventory identities.
use super::*;

const METHODS: &[(&str, &str, &str)] = &[
    (
        "longitudinal.gee",
        "Generalized estimating equations (GEE)",
        "广义估计方程 GEE",
    ),
    (
        "mixed.hlm",
        "Hierarchical linear model (HLM)",
        "多层线性模型 HLM",
    ),
    (
        "mixed.lmm",
        "Linear mixed model (LMM)",
        "线性混合效应模型 LMM",
    ),
    (
        "mixed.glmm",
        "Generalized linear mixed model (GLMM)",
        "广义线性混合模型 GLMM",
    ),
    (
        "mixed.random_intercept",
        "Random-intercept model",
        "随机截距模型",
    ),
    ("mixed.random_slope", "Random-slope model", "随机斜率模型"),
    (
        "mixed.crossed_effects",
        "Crossed random-effects model",
        "交叉随机效应模型",
    ),
    (
        "mixed.logistic",
        "Logistic mixed model",
        "Logistic 混合模型",
    ),
    ("mixed.poisson", "Poisson mixed model", "Poisson 混合模型"),
    (
        "mixed.negative_binomial",
        "Negative-binomial mixed model",
        "负二项混合模型",
    ),
];

fn label_series() -> Result<TypeExpr, BuiltinAssemblyError> {
    normalize_type_expr(TypeExpr::Union(
        [
            "core.numeric",
            "core.binary",
            "core.categorical",
            "core.ordinal",
            "core.text",
            "core.identifier",
        ]
        .iter()
        .map(|id| concrete(id).map(data_series_type))
        .collect::<Result<Vec<_>, _>>()?,
    ))
    .map_err(|e| BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
        context: "longitudinal grouping",
        value: e.to_string().into(),
    })
}

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, en, zh) in METHODS {
        let id = format!("yssbi.statistics.{method}");
        let gee = method == "longitudinal.gee";
        let generalized = matches!(
            method,
            "mixed.glmm" | "mixed.logistic" | "mixed.poisson" | "mixed.negative_binomial"
        );
        let response = normalize_type_expr(TypeExpr::Union(vec![
            series_type()?,
            data_series_type(concrete("core.binary")?),
        ]))
        .map_err(|e| BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
            context: "longitudinal response",
            value: e.to_string().into(),
        })?;
        let mut ports = vec![
            data_input("y", "Y", response)?,
            bounded_user_data_input("x", "X", series_type()?, 0, None)?,
            bounded_user_data_input(
                "groups",
                "Grouping factor",
                label_series()?,
                if method == "mixed.crossed_effects" {
                    2
                } else {
                    1
                },
                if matches!(method, "mixed.hlm" | "mixed.crossed_effects") {
                    None
                } else {
                    Some(1)
                },
            )?,
        ];
        if matches!(method, "mixed.lmm" | "mixed.random_slope") {
            ports.push(bounded_user_data_input(
                "random_predictors",
                "Random slope",
                series_type()?,
                if method == "mixed.random_slope" { 1 } else { 0 },
                None,
            )?);
        }
        ports.push(data_output("result", "Result", report_type()?)?);
        let mut parameters = vec![
            toggle_parameter("constant", true)?,
            positive_integer_parameter("max_iterations", 500)?,
            tolerance_parameter("0.0000001")?,
        ];
        if gee || method == "mixed.glmm" {
            parameters.push(choice_parameter(
                "longitudinal_family",
                if gee { "gaussian" } else { "binomial" },
                if gee {
                    &["gaussian", "binomial", "poisson"]
                } else {
                    &["binomial", "poisson", "negative_binomial"]
                },
            )?);
        }
        if gee {
            parameters.push(choice_parameter(
                "working_correlation",
                "exchangeable",
                &["independence", "exchangeable"],
            )?);
        } else if !generalized {
            parameters.push(choice_parameter(
                "mixed_estimation",
                "reml",
                &["reml", "ml"],
            )?);
        }
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.longitudinal", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, ports, vec![], vec![])?,
                parameters: assembled_parameters(&id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: execution(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            &id,
        ));
        let aliases: &'static [&'static str] = match method {
            "longitudinal.gee" => &["longitudinal.gee", "GEE", "广义估计方程"],
            "mixed.hlm" => &["mixed.hlm", "HLM", "多层线性模型", "hierarchical"],
            "mixed.lmm" => &["mixed.lmm", "LMM", "线性混合效应模型"],
            "mixed.glmm" => &["mixed.glmm", "GLMM", "广义线性混合模型"],
            "mixed.random_intercept" => &["mixed.random_intercept", "随机截距模型"],
            "mixed.random_slope" => &["mixed.random_slope", "随机斜率模型"],
            "mixed.crossed_effects" => &["mixed.crossed_effects", "交叉随机效应模型"],
            "mixed.logistic" => &["mixed.logistic", "Logistic混合模型"],
            "mixed.poisson" => &["mixed.poisson", "Poisson混合模型"],
            _ => &["mixed.negative_binomial", "负二项混合模型", "NB2"],
        };
        for (locale, title, help) in [
            (
                "en-US",
                en,
                "Estimate aligned grouped observations. See help for response families, random-effect design and inference.",
            ),
            (
                "zh-CN",
                zh,
                "拟合对齐的分组观测；响应分布、随机效应设计与推断口径见节点帮助。",
            ),
        ] {
            fragment.messages.extend([
                (locale, node_key_text(&id, "title"), Text(title)),
                (locale, node_key_text(&id, "documentation"), Text(help)),
                (locale, node_key_text(&id, "aliases"), Aliases(aliases)),
            ]);
        }
    }
    for (key, en, zh, help_en, help_zh) in [
        (
            "longitudinal_family",
            "Response family",
            "响应分布",
            "Gaussian identity, binary logit, Poisson log, or NB2 log, as offered by this node.",
            "按节点选择 Gaussian 恒等、二项 logit、Poisson log 或 NB2 log。",
        ),
        (
            "working_correlation",
            "Working correlation",
            "工作相关结构",
            "Exchangeable (default) or independence; inference uses a cluster-robust sandwich covariance.",
            "默认可交换，亦可选独立；推断使用按组稳健的三明治协方差。",
        ),
        (
            "mixed_estimation",
            "Mixed-model estimation",
            "混合模型估计",
            "REML (default) or maximum likelihood. Use ML when comparing different fixed-effect designs.",
            "默认 REML，亦可选最大似然 ML；比较不同固定效应设计时使用 ML。",
        ),
    ] {
        for (locale, title, help) in [("en-US", en, help_en), ("zh-CN", zh, help_zh)] {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn longitudinal_catalog_retains_all_identities_and_complete_bilingual_help() {
        let system = crate::build_builtin_node_system().unwrap();
        for locale in ["en-US", "zh-CN"] {
            let catalog = system.catalog.localize(&system.registry, locale);
            for &(suffix, _, _) in METHODS {
                let id = format!("yssbi.statistics.{suffix}");
                let protocol = system
                    .registry
                    .protocol(&id.as_str().parse().unwrap())
                    .unwrap();
                let item = catalog
                    .items
                    .iter()
                    .find(|i| i.node_type_id.as_ref() == id)
                    .unwrap();
                assert_eq!(item.category_id.as_ref(), "statistics.longitudinal");
                let help = item.documentation.as_deref().unwrap();
                assert!(
                    help.contains("$$")
                        && help.contains("max_iterations")
                        && help.contains("group_labels"),
                    "{id}: {locale}"
                );
                assert!(!help.contains("范围待确认"));
                assert_eq!(
                    protocol
                        .interface
                        .ports
                        .iter()
                        .filter(|p| p.direction == PortDirection::Output)
                        .count(),
                    1
                );
                assert_eq!(
                    protocol.interface.ports.last().unwrap().key.as_str(),
                    "result"
                );
                let default = |key: &str| {
                    protocol
                        .parameters
                        .iter()
                        .find(|p| p.key.as_str() == key)
                        .unwrap()
                        .default_value
                        .as_ref()
                        .unwrap()
                        .value
                        .clone()
                };
                assert_eq!(default("constant"), DataValue::Bool(true));
                assert_eq!(default("max_iterations"), DataValue::Integer(500));
                if suffix == "longitudinal.gee" {
                    assert_eq!(
                        default("working_correlation"),
                        DataValue::String("exchangeable".into())
                    );
                }
            }
        }
    }
}
