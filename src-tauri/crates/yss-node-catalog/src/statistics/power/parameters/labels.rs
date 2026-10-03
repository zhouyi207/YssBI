use super::*;
pub(super) fn localize(
    fragment: &mut ProviderFragment,
    id: &str,
    params: &mut [Parameter],
) -> Result<(), BuiltinAssemblyError> {
    for p in params {
        let (en, zh, eh, zhh) = match p.key.as_str() {
            "solve_for" => (
                "Calculate",
                "计算目标",
                "power uses sample_size; sample_size uses target_power.",
                "power 按给定样本量算功效；sample_size 按目标功效求样本量。",
            ),
            "sample_size" => (
                "Sample size in design units",
                "按设计单位的样本量",
                "Default 100. May mean per group, pairs or clusters per group; consult this node's help.",
                "默认 100；按节点可能表示每组人数、配对数或每组群数，详见帮助。",
            ),
            "target_power" => (
                "Target power",
                "目标功效",
                "Strictly between 0 and 1; default 0.8. Used only when solving for sample size.",
                "0 与 1 之间，默认 0.8；仅求样本量时使用。",
            ),
            "alpha" => (
                "Significance level",
                "显著性水平",
                "Strictly between 0 and 1; default 0.05. TOST uses this level for each one-sided test.",
                "0 与 1 之间，默认 0.05；TOST 的每个单侧检验均使用此水平。",
            ),
            "alternative" => (
                "Alternative",
                "备择方向",
                "two_sided, greater or less relative to the node's null value.",
                "相对于零假设取 two_sided（双侧）、greater（大于）或 less（小于）。",
            ),
            "effect_size" => (
                "Standardized mean effect",
                "标准化均值效应",
                "Signed mean difference divided by the relevant population SD; paired tests use SD of differences.",
                "均值差除以对应总体标准差，保留正负号；配对检验使用差值的标准差。",
            ),
            "design" => (
                "Mean design",
                "均值设计",
                "independent: equal-size equal-variance groups; one_sample: one mean.",
                "independent：等样本量等方差两组；one_sample：单样本均值。",
            ),
            "variance_ratio" => (
                "Alternative / null variance",
                "备择方差／零假设方差",
                "Positive variance ratio, default 1.5.",
                "正方差比，默认 1.5。",
            ),
            "null_proportion" => (
                "Null proportion",
                "零假设比例",
                "Strictly between 0 and 1, default 0.5.",
                "0 与 1 之间，默认 0.5。",
            ),
            "proportion" => (
                "Alternative proportion",
                "备择比例",
                "From 0 through 1, default 0.6.",
                "0 至 1，默认 0.6。",
            ),
            "proportion1" => (
                "Group 1 proportion",
                "第 1 组比例",
                "From 0 through 1, default 0.6.",
                "0 至 1，默认 0.6。",
            ),
            "proportion2" => (
                "Group 2 proportion",
                "第 2 组比例",
                "From 0 through 1, default 0.4.",
                "0 至 1，默认 0.4。",
            ),
            "null_correlation" => (
                "Null correlation",
                "零假设相关系数",
                "Strictly between -1 and 1, default 0.",
                "-1 与 1 之间，默认 0。",
            ),
            "correlation" => (
                "Alternative correlation",
                "备择相关系数",
                "Strictly between -1 and 1, default 0.3.",
                "-1 与 1 之间，默认 0.3。",
            ),
            "effect_f" => (
                "Cohen f",
                "Cohen f 效应量",
                "Nonnegative ANOVA f, default 0.25.",
                "非负 ANOVA 效应量 f，默认 0.25。",
            ),
            "effect_f_squared" => (
                "Cohen f squared",
                "Cohen f² 效应量",
                "Nonnegative overall R²/(1-R²), default 0.15.",
                "非负整体 R²/(1-R²)，默认 0.15。",
            ),
            "groups" => (
                "ANOVA groups",
                "ANOVA 组数",
                "Integer at least 2, default 3.",
                "至少 2 组，默认 3。",
            ),
            "predictors" => (
                "Regression predictors",
                "回归自变量数",
                "Positive integer excluding the intercept, default 3.",
                "不含截距的正整数，默认 3。",
            ),
            "baseline_rate" => (
                "Baseline event rate",
                "基准事件率",
                "Positive rate per unit exposure, default 1.",
                "单位暴露量的正事件率，默认 1。",
            ),
            "rate_ratio" => (
                "Rate ratio",
                "发生率比",
                "Positive group 1 / group 0 rate ratio, default 1.5.",
                "第 1 组／基准组的正发生率比，默认 1.5。",
            ),
            "exposure" => (
                "Exposure per subject",
                "每人暴露量",
                "Same positive exposure in both groups, default 1.",
                "两组相同的正暴露量，默认 1。",
            ),
            "baseline_probability" => (
                "Baseline outcome probability",
                "基准结果概率",
                "Strictly between 0 and 1, default 0.2.",
                "0 与 1 之间，默认 0.2。",
            ),
            "odds_ratio" => (
                "Odds ratio",
                "优势比 OR",
                "Positive odds ratio, default 1.5.",
                "正优势比，默认 1.5。",
            ),
            "hazard_ratio" => (
                "Hazard ratio",
                "风险比 HR",
                "Positive proportional hazard ratio, default 0.7.",
                "比例风险假设下的正风险比，默认 0.7。",
            ),
            "event_fraction" => (
                "Expected event fraction",
                "预期事件比例",
                "Greater than 0 through 1; expected observed events / enrolled subjects; default 0.5.",
                "大于 0 且不超过 1；预期观测事件数／入组人数，默认 0.5。",
            ),
            "predictor_variance" => (
                "Predictor variance",
                "预测变量方差",
                "Positive population variance, default 0.25; assumes no adjustment for other predictors.",
                "正总体方差，默认 0.25；假定无其他协变量调整。",
            ),
            "allocation" => (
                "Treatment allocation fraction",
                "处理组分配比例",
                "Strictly between 0 and 1, default 0.5.",
                "0 与 1 之间，默认 0.5。",
            ),
            "cluster_size" => (
                "Subjects per cluster",
                "每群人数",
                "Positive integer, equal cluster sizes, default 20.",
                "正整数、等群大小，默认 20。",
            ),
            "icc" => (
                "Intraclass correlation",
                "组内相关系数 ICC",
                "From 0 through 1, default 0.05.",
                "0 至 1，默认 0.05。",
            ),
            "difference" => (
                "True standardized difference",
                "真实标准化差异",
                "Mean difference / common known SD; default 0.",
                "均值差／共同已知标准差，默认 0。",
            ),
            "margin" => (
                "Standardized margin",
                "标准化界值",
                "Positive margin divided by common known SD, default 0.3.",
                "界值除以共同已知标准差，必须为正，默认 0.3。",
            ),
            _ => (
                "Higher outcome is better",
                "结果越高越好",
                "True by default; false reverses the direction for noninferiority.",
                "默认 true；false 反转非劣效检验方向。",
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
