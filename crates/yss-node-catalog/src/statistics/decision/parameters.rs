//! Localized decision parameter labels; numerical rules stay in SCI.
use super::*;
pub(super) fn localize(
    fragment: &mut ProviderFragment,
    id: &str,
    parameters: &mut [Parameter],
) -> Result<(), BuiltinAssemblyError> {
    for parameter in parameters {
        let key = parameter.key.as_str();
        let (en, zh, eh, zhh) = match key {
            "full_score" => (
                "Full score",
                "满分值",
                "Maximum possible rating, used for the full-score percentage. Default 5.",
                "量表允许的满分值，用于满分频率，默认 5；不是本次观测最大值。",
            ),
            "random_index" => (
                "Random consistency index",
                "随机一致性 RI",
                "0 selects the reference table for orders 3–15; a positive value supplies an external RI.",
                "0 表示 3–15 阶自动查表；正数表示使用自行指定的 RI。其他阶数仍计算权重与 CI。",
            ),
            "influence_normalization" => (
                "Influence normalization",
                "直接影响标准化",
                "Divide by the largest row/column sum, or use the supplied direct-influence scale.",
                "除以最大行和或列和，或使用输入的直接影响尺度。",
            ),
            "attenuation" => (
                "Influence attenuation",
                "影响衰减系数",
                "Explicit multiplier in (0,1], default 1; the resulting spectral radius must be below 1.",
                "标准化后乘以此系数，范围 (0,1]，默认 1；所得矩阵的谱半径须小于 1。",
            ),
            "fuzzy_operator" => (
                "Membership operator",
                "隶属度合成算子",
                "Weighted sum, max-min, max-product, or bounded sum-min.",
                "选择乘积求和、最小值取大、乘积取大或最小值有界求和。",
            ),
            "combination_size" => (
                "Combination size",
                "组合大小",
                "Number of options to select; every subset of this size is evaluated.",
                "组合中选项的数量；精确比较该大小的所有组合。",
            ),
            "range_definition" => (
                "Acceptable range definition",
                "接受价格区间定义",
                "Original van Westendorp or the narrower four-curve convention.",
                "选择原始 Van Westendorp 定义或较窄的四曲线交点定义。",
            ),
            "cost_criteria" => (
                "Cost criteria",
                "负向指标",
                "One-based criterion positions where smaller values are preferred; empty means all benefits.",
                "从 1 开始的负向指标位置；留空表示全部越大越好。",
            ),
            "weight_method" => (
                "Weight method",
                "赋权方法",
                "Choose an objective method, equal weights, or an explicit connected weight vector.",
                "选择客观赋权、等权，或使用单独连接的权重向量。",
            ),
            "normalization" => (
                "Score normalization",
                "评分标准化",
                "Min–max, vector norm, or unchanged values; objective weights retain their own defined preprocessing.",
                "极差、向量归一化或原值；客观赋权仍使用各自定义的预处理。",
            ),
            "resolution" => (
                "Grey resolution coefficient",
                "灰色分辨系数",
                "Greater than 0 and at most 1; default 0.5.",
                "大于 0 且不超过 1，默认 0.5。",
            ),
            "majority_weight" => (
                "Majority preference",
                "多数效用权重",
                "VIKOR balance v in [0,1], default 0.5.",
                "VIKOR 权衡系数 v，范围 [0,1]，默认 0.5。",
            ),
            "rescale" => (
                "Min–max normalization",
                "极差标准化",
                "Normalize before obstacle attribution; turn off for already normalized [0,1] values.",
                "计算障碍度前逐列极差标准化；已为 [0,1] 数值时可关闭。",
            ),
            _ => unreachable!(),
        };
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
    Ok(())
}
