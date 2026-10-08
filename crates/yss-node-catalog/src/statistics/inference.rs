//! Explicit estimand/SE intervals and independent-group post-hoc contrasts.
use super::*;
const NODES: &[(&str, &str, &str)] = &[
    (
        "inference.confidence_interval",
        "Confidence intervals",
        "置信区间 CI",
    ),
    (
        "posthoc.multiple_comparisons",
        "Post-hoc multiple comparisons",
        "事后多重比较",
    ),
    (
        "inference.cluster_robust",
        "Cluster-robust standard errors",
        "聚类稳健标准误",
    ),
    (
        "postestimation.adjusted_predictions",
        "Adjusted predictions",
        "调整预测",
    ),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(suffix, en, zh) in NODES {
        let id = format!("yssbi.statistics.{suffix}");
        let interval = suffix == "inference.confidence_interval";
        let cluster = suffix == "inference.cluster_robust";
        let prediction = suffix == "postestimation.adjusted_predictions";
        let mut ports = if cluster {
            vec![
                data_input("y", "Y", series_type()?)?,
                data_input("clusters", "Clusters", label_series()?)?,
                bounded_user_data_input("x", "X", series_type()?, 1, None)?,
            ]
        } else if prediction {
            vec![data_input(
                "model",
                "Fitted model",
                fitted_regression_type()?,
            )?]
        } else if interval {
            vec![
                data_input("estimates", "Estimates", series_type()?)?,
                data_input("standard_errors", "Standard errors", series_type()?)?,
            ]
        } else {
            vec![
                data_input("y", "Y", series_type()?)?,
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
                let mut port = fixed_numeric_table(
                    "comparisons",
                    "Comparisons",
                    &[
                        "group_a",
                        "group_b",
                        "group_a_label",
                        "group_b_label",
                        "estimate",
                        "standard_error",
                        "degrees_of_freedom",
                        "statistic",
                        "p_value",
                        "adjusted_p_value",
                        "lower",
                        "upper",
                    ],
                )?;
                if let Some(SchemaExpr::Fixed { fields }) = &mut port.schema {
                    for field in &mut fields[2..4] {
                        field.scalar_type =
                            RelationalScalarType::Known(yss_data_contract::SemanticType::Text);
                    }
                }
                port
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
        for parameter in &mut parameters {
            let key = parameter.key.as_str();
            let (en, zh, eh, zhh) = match key {
                "confidence_level" => (
                    "Confidence level",
                    "置信水平",
                    "Strictly between 0 and 1; default 0.95.",
                    "严格介于 0 和 1，默认 0.95。",
                ),
                "degrees_of_freedom" => (
                    "Reference degrees of freedom",
                    "参考自由度",
                    "0 selects normal intervals; a positive value selects Student t intervals.",
                    "0 表示正态区间；正值表示 Student t 区间。",
                ),
                "equal_variances" => (
                    "Common group variance",
                    "组间等方差",
                    "Use the common ANOVA residual variance; turn off for pairwise Welch inference.",
                    "使用 ANOVA 合并残差方差；关闭时采用两两 Welch 推断。",
                ),
                "adjustment" => (
                    "Multiplicity adjustment",
                    "多重性校正",
                    "Holm and Bonferroni control family-wise error; only Bonferroni also adjusts intervals.",
                    "Holm 与 Bonferroni 控制族错误率；仅 Bonferroni 同时校正区间。",
                ),
                "constant" => (
                    "Include intercept",
                    "包含截距",
                    "Fit an intercept; predictors must have full column rank.",
                    "拟合截距；自变量须列满秩。",
                ),
                "evaluation" => (
                    "Evaluation",
                    "预测口径",
                    "Average individual adjusted predictions or predict at covariate means.",
                    "对各行调整预测取均值，或在协变量均值处预测。",
                ),
                "at" => (
                    "Covariate settings",
                    "协变量设定",
                    "Optional comma-separated assignments, e.g. x1=2,x2=0; use fitted coefficient names.",
                    "可选逗号分隔赋值，如 x1=2,x2=0；使用拟合系数的名称。",
                ),
                _ => unreachable!(),
            };
            parameter.title_key = node_key(&id, &format!("parameters.{key}.title"))?;
            parameter.description_key =
                Some(node_key(&id, &format!("parameters.{key}.description"))?);
            for (locale, title, help) in [("en-US", en, eh), ("zh-CN", zh, zhh)] {
                fragment.messages.extend([
                    (
                        locale,
                        parameter.title_key.as_str().to_string(),
                        Text(title),
                    ),
                    (
                        locale,
                        parameter
                            .description_key
                            .as_ref()
                            .unwrap()
                            .as_str()
                            .to_string(),
                        Text(help),
                    ),
                ]);
            }
        }
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
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
