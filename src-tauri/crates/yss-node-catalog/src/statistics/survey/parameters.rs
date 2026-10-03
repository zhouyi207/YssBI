use super::*;
pub(super) fn build(
    fragment: &mut ProviderFragment,
    id: &str,
    method: &str,
    regression: bool,
) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let mut parameters = if method == "weights" {
        vec![choice_parameter(
            "input_kind",
            "weights",
            &["weights", "inclusion_probabilities"],
        )?]
    } else {
        vec![choice_parameter(
            "lonely_psu",
            "fail",
            &["fail", "certainty"],
        )?]
    };
    if regression {
        parameters.extend([
            toggle_parameter("constant", true)?,
            positive_integer_parameter("max_iterations", 500)?,
            tolerance_parameter("0.0000001")?,
        ]);
    } else if method != "weights" {
        parameters.push(choice_parameter(
            "statistic",
            "mean",
            &["mean", "proportion"],
        )?);
    }
    for p in &mut parameters {
        let (en, zh, eh, zhh) = match p.key.as_str() {
            "input_kind" => (
                "Input meaning",
                "输入含义",
                "Positive sampling weights, or inclusion probabilities in (0,1] converted to 1/probability.",
                "正抽样权重，或 (0,1] 内的纳入概率（转换为概率的倒数）。",
            ),
            "lonely_psu" => (
                "Single-PSU stratum",
                "单 PSU 层处理",
                "fail rejects unidentified variance; certainty assigns zero variance only for known certainty strata.",
                "fail 拒绝无法估计的层内方差；certainty 仅用于已知必选层，将其方差贡献设为零。",
            ),
            "statistic" => (
                "Statistic",
                "统计量",
                "Mean, or proportion for a 0/1 response.",
                "均值，或 0/1 响应的比例。",
            ),
            "constant" => (
                "Include intercept",
                "包含截距",
                "Include a constant term in the regression.",
                "在回归方程中加入截距。",
            ),
            "max_iterations" => (
                "Maximum iterations",
                "最大迭代次数",
                "Positive numerical convergence budget, default 500.",
                "正整数，数值收敛迭代预算，默认 500。",
            ),
            _ => (
                "Convergence tolerance",
                "收敛容差",
                "Relative coefficient convergence tolerance from 1e-12 through 0.01; default 1e-7.",
                "系数相对收敛容差，1e-12 至 0.01，默认 1e-7。",
            ),
        };
        p.title_key = node_key(id, &format!("parameters.{}.title", p.key.as_str()))?;
        p.description_key = Some(node_key(
            id,
            &format!("parameters.{}.description", p.key.as_str()),
        )?);
        for (locale, title, help) in [("en-US", en, eh), ("zh-CN", zh, zhh)] {
            fragment.messages.extend([
                (locale, p.title_key.as_str().to_string(), Text(title)),
                (
                    locale,
                    p.description_key.as_ref().unwrap().as_str().to_string(),
                    Text(help),
                ),
            ]);
        }
    }
    Ok(parameters)
}
