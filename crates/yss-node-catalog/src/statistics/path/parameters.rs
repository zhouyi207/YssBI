use super::*;
pub(super) fn localize(
    fragment: &mut ProviderFragment,
    id: &str,
    parameters: &mut [Parameter],
) -> Result<(), BuiltinAssemblyError> {
    for p in parameters {
        let (en, zh, eh, zhh) = match p.key.as_str() {
            "probe_sd" => (
                "Probe distance (SD)",
                "探查距离（标准差倍数）",
                "Positive multiplier for mean ± sample SD probes; the mean is included.",
                "使用均值及均值上下若干个样本标准差作为探查点，倍数须为正。",
            ),
            "replications" => (
                "Bootstrap replications",
                "Bootstrap 重采样次数",
                "0 disables inference; otherwise at least 2. Default 1000; more replications improve percentile stability.",
                "0 只计算点估计，否则至少 2 次；默认 1000 次，增加次数可提高百分位区间稳定性。",
            ),
            "seed" => (
                "Random seed",
                "随机种子",
                "Nonnegative seed for reproducible paired-row resampling.",
                "非负整数，用于复现整行配对重采样。",
            ),
            "stage" => (
                "Moderated path",
                "被调节路径",
                "first: X → M; second: M → Y. W enters both equations as a main effect.",
                "first：X → M；second：M → Y。W 主效应进入两个方程。",
            ),
            _ => (
                "Path equations",
                "路径方程",
                "Use x1, x2, … in input order; separate equations by semicolons or newlines. Example: x2 ~ x1; x3 ~ x1 + x2.",
                "按输入顺序用 x1、x2……引用变量，分号或换行分隔方程，例如 x2 ~ x1; x3 ~ x1 + x2。",
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
    Ok(())
}
