//! Method-specific settings and their bilingual descriptions.
use super::*;

pub(super) fn build(
    id: &str,
    method: &str,
    fragment: &mut ProviderFragment,
) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let mut parameters = Vec::new();
    if !matches!(
        method,
        "combine_p" | "cochran_q" | "i_squared" | "tau_squared" | "begg"
    ) {
        parameters.push(decimal_parameter("confidence_level", "0.95")?);
    }
    let measures: Option<(&str, &[&str])> = match method {
        "continuous" => Some(("mean_difference", &["mean_difference", "hedges_g"])),
        "binary" => Some((
            "log_odds_ratio",
            &["log_odds_ratio", "log_risk_ratio", "risk_difference"],
        )),
        "single_proportion" => Some((
            "logit_proportion",
            &["proportion", "logit_proportion", "arcsine_proportion"],
        )),
        _ => None,
    };
    if let Some((default, values)) = measures {
        parameters.push(choice_parameter("effect_measure", default, values)?);
    }
    if matches!(method, "binary" | "single_proportion") {
        parameters.push(decimal_parameter("continuity_correction", "0.5")?);
    }
    if method == "or_hr" {
        parameters.push(decimal_parameter("source_confidence_level", "0.95")?);
    }
    if method == "combine_p" {
        parameters.push(choice_parameter(
            "p_method",
            "fisher",
            &["fisher", "stouffer"],
        )?);
    }
    if matches!(method, "random_effect" | "tau_squared") {
        parameters.push(choice_parameter(
            "estimator",
            "paule_mandel",
            &["der_simonian_laird", "paule_mandel"],
        )?);
    } else if matches!(
        method,
        "inverse_variance" | "regression" | "leave_one_out" | "sensitivity" | "forest" | "funnel"
    ) {
        parameters.push(choice_parameter(
            "estimator",
            if method == "inverse_variance" {
                "fixed"
            } else {
                "paule_mandel"
            },
            &["fixed", "der_simonian_laird", "paule_mandel"],
        )?);
    }
    if matches!(
        method,
        "fixed_effect"
            | "random_effect"
            | "inverse_variance"
            | "regression"
            | "leave_one_out"
            | "sensitivity"
            | "forest"
    ) {
        parameters.push(choice_parameter(
            "inference",
            "wald",
            &["wald", "knapp_hartung"],
        )?);
    }
    if method == "forest" {
        parameters.push(toggle_parameter("exponentiate", false)?);
    }
    for parameter in &mut parameters {
        let key = parameter.key.as_str();
        let (en, zh, eh, zhh) = labels(key);
        parameter.title_key = node_key(id, &format!("parameters.{key}.title"))?;
        parameter.description_key = Some(node_key(id, &format!("parameters.{key}.description"))?);
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
    Ok(parameters)
}
fn labels(key: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match key {
        "confidence_level" => (
            "Confidence level",
            "置信水平",
            "Strictly between 0 and 1; default 0.95.",
            "严格介于 0 和 1，默认 0.95。",
        ),
        "effect_measure" => (
            "Effect measure",
            "效应量口径",
            "Use the same outcome, contrast direction and analysis scale across studies.",
            "所有研究应使用相同结局、对照方向与分析尺度。",
        ),
        "continuity_correction" => (
            "Continuity correction",
            "连续性校正",
            "Nonnegative; default 0.5, applied only to studies with a zero cell or a boundary proportion.",
            "非负，默认 0.5；仅在某格为零或比例处于边界时校正。",
        ),
        "source_confidence_level" => (
            "Reported CI level",
            "原报告置信水平",
            "Confidence level of the supplied ratio intervals; default 0.95.",
            "输入 OR/HR 区间的置信水平，默认 0.95。",
        ),
        "p_method" => (
            "Combination method",
            "合并方法",
            "Fisher combines independent p-values; Stouffer combines directional one-sided p-values with optional positive weights.",
            "Fisher 合并独立 P 值；Stouffer 合并方向一致的单侧 P 值，可连接正权重。",
        ),
        "estimator" => (
            "Pooling / heterogeneity method",
            "合并与异质性方法",
            "Fixed effect, nonnegative DerSimonian-Laird moments, or iterative Paule-Mandel moments.",
            "固定效应、非负 DerSimonian-Laird 矩估计或迭代 Paule-Mandel 矩估计。",
        ),
        "inference" => (
            "Coefficient inference",
            "系数推断",
            "Wald uses known-variance normal inference; Knapp-Hartung estimates residual scale and uses t(k-p).",
            "Wald 使用已知方差的正态推断；Knapp-Hartung 估计残差尺度并使用 t(k-p)。",
        ),
        "exponentiate" => (
            "Exponentiate effects",
            "效应取指数",
            "Display log-ratio effects as ratios; default off. Pooling always uses the input analysis scale.",
            "将对数比值显示为比值，默认关闭；合并始终使用输入的分析尺度。",
        ),
        _ => unreachable!("meta parameter"),
    }
}
