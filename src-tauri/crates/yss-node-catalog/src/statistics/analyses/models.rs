//! Additional model diagnostics with explicit fitted-model or aligned-series inputs.
use super::super::*;

const METHODS: &[(&str, &str, &str)] = &[
    ("collinearity", "Collinearity diagnostics", "共线性分析"),
    ("nri_idi", "NRI and IDI", "NRI 和 IDI"),
    (
        "harman",
        "Harman single-factor diagnostic",
        "共同方法偏差（Harman 单因子）",
    ),
    ("residual", "Residual diagnostics", "残差分析"),
    ("cooks_distance", "Cook's distance", "Cook 距离"),
    (
        "aic",
        "Akaike information criterion (AIC)",
        "AIC 赤池信息准则",
    ),
    (
        "bic",
        "Bayesian information criterion (BIC)",
        "BIC 贝叶斯信息准则",
    ),
    ("lr", "Likelihood-ratio test", "似然比检验（LR）"),
    ("score_lm", "Score / LM test", "Score / LM 检验"),
    (
        "nested_comparison",
        "Nested-model comparison",
        "嵌套模型比较",
    ),
    (
        "ph",
        "Proportional-hazards score test",
        "比例风险 PH 假设检验",
    ),
];
pub(super) fn implemented(id: &str) -> bool {
    id.strip_prefix("yssbi.statistics.diagnostic.")
        .is_some_and(|method| METHODS.iter().any(|m| m.0 == method))
}
fn binary_series() -> Result<TypeExpr, BuiltinAssemblyError> {
    normalize_type_expr(TypeExpr::Union(vec![
        series_type()?,
        data_series_type(concrete("core.binary")?),
    ]))
    .map_err(|e| BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
        context: "diagnostic outcome",
        value: e.to_string().into(),
    })
}

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, en, zh) in METHODS {
        let id = format!("yssbi.statistics.diagnostic.{method}");
        let mut ports = match method {
            "collinearity" | "harman" => vec![bounded_user_data_input(
                "variables",
                "Variable",
                series_type()?,
                if method == "harman" { 2 } else { 1 },
                None,
            )?],
            "nri_idi" => vec![
                data_input("outcome", "Binary outcome", binary_series()?)?,
                data_input("reference", "Reference probability", series_type()?)?,
                data_input("new", "New probability", series_type()?)?,
            ],
            "ph" => vec![
                data_input("time", "Time", series_type()?)?,
                data_input("event", "Event", binary_series()?)?,
                bounded_user_data_input("predictors", "Predictor", series_type()?, 1, None)?,
            ],
            "lr" | "score_lm" | "nested_comparison" => vec![
                data_input("restricted", "Restricted model", fitted_regression_type()?)?,
                data_input("full", "Full model", fitted_regression_type()?)?,
            ],
            "residual" | "cooks_distance" => vec![data_input(
                "model",
                "Model",
                concrete("statistics.model.linear")?,
            )?],
            _ => vec![data_input("model", "Model", fitted_regression_type()?)?],
        };
        ports.push(data_output("result", "Result", report_type()?)?);
        if matches!(method, "residual" | "cooks_distance") {
            ports.push(fixed_numeric_table(
                "observations",
                "Observation diagnostics",
                &[
                    "observation",
                    "fitted",
                    "residual",
                    "weighted_residual",
                    "leverage",
                    "standardized_residual",
                    "studentized_residual",
                    "cooks_distance",
                ],
            )?);
        }
        let mut parameters = match method {
            "collinearity" => vec![toggle_parameter("constant", true)?],
            "nri_idi" => vec![
                choice_parameter("nri_mode", "continuous", &["continuous", "categorical"])?,
                parameter(
                    "risk_thresholds",
                    series_type()?,
                    ParameterEditorSpec::Auto,
                    DataValue::List(vec![]),
                    vec![],
                )?,
            ],
            "ph" => vec![
                choice_parameter("survival_ties", "efron", &["efron", "breslow"])?,
                choice_parameter("time_transform", "rank", &["rank", "log", "identity"])?,
                positive_integer_parameter("max_iterations", 500)?,
                tolerance_parameter("0.0000001")?,
            ],
            _ => vec![],
        };
        for parameter in &mut parameters {
            let key = parameter.key.as_str();
            let (en, zh, eh, zh_help) = parameter_text(key);
            parameter.title_key = node_key(&id, &format!("parameters.{key}.title"))?;
            parameter.description_key =
                Some(node_key(&id, &format!("parameters.{key}.description"))?);
            for (locale, title, help) in [("en-US", en, eh), ("zh-CN", zh, zh_help)] {
                fragment.messages.extend([
                    (locale, parameter.title_key.as_str().to_owned(), Text(title)),
                    (
                        locale,
                        parameter
                            .description_key
                            .as_ref()
                            .unwrap()
                            .as_str()
                            .to_owned(),
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
                    category_id: sid("statistics.diagnostics", NodeCategoryId::new)?,
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
fn parameter_text(key: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match key {
        "constant" => (
            "Include intercept",
            "包含截距",
            "Center predictors and include an intercept by default; disabling uses the uncentered design.",
            "默认中心化自变量并包含截距；关闭后使用未中心化设计。",
        ),
        "nri_mode" => (
            "Reclassification mode",
            "重分类方式",
            "Continuous compares probabilities directly; categorical compares configured risk groups.",
            "连续型直接比较预测概率；分类型比较配置的风险组。",
        ),
        "risk_thresholds" => (
            "Risk thresholds",
            "风险分界点",
            "Empty for continuous NRI; categorical NRI requires increasing cut points strictly between 0 and 1. Equality enters the higher group.",
            "连续型留空；分类型填写严格递增且介于 0 和 1 之间的分界点，等于分界点归入较高组。",
        ),
        "survival_ties" => (
            "Tied events",
            "并列事件",
            "Use Efron (default) or Breslow partial likelihood consistently for Cox fitting and the score test.",
            "Cox 拟合和 Score 检验一致采用 Efron（默认）或 Breslow 偏似然。",
        ),
        "time_transform" => (
            "Time transform",
            "时间变换",
            "Rank (default), log(time), or time; tests covariate interactions with transformed event time.",
            "默认使用时间秩，也可选对数时间或原始时间；检验协变量与变换后事件时间的交互效应。",
        ),
        "max_iterations" => (
            "Maximum iterations",
            "最大迭代次数",
            "Positive iteration budget for Cox fitting; default 500.",
            "Cox 拟合的正整数迭代预算，默认 500。",
        ),
        "tolerance" => (
            "Convergence tolerance",
            "收敛容差",
            "Default 1e-7; supported numerical range 1e-12 to 0.01.",
            "默认 1e-7；数值收敛范围为 1e-12 至 0.01。",
        ),
        _ => unreachable!("diagnostic parameter"),
    }
}
