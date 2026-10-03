# 复杂抽样均值／比例

连接 response 响应和 weights 抽样权重，可连接一列 strata 层标签及一列 clusters 初级抽样单元（PSU）标签。

未提供 strata 时视为一层；未提供 clusters 时每行是一个独立 PSU。不同层内相同的 PSU 标签分别识别。列须对齐且无缺失，数值有限，抽样权重严格为正。不自动删除记录。

使用单阶段、有放回近似的 Taylor 线性化方差，不包含有限总体修正、多阶段设计、复制权重或子总体分析。第 h 层有 m_h 个 PSU，t_hj 为该 PSU 的得分之和，则方差汇总各层 `m_h/(m_h−1) × Σ(t_hj−层均值得分)²`；回归使用外积矩阵并左右乘信息矩阵逆。设计自由度为 PSU 总数减层数。

`lonely_psu=fail` 默认拒绝单 PSU 层；仅当该层确为必选且无未计入的后续抽样阶段时使用 `certainty`，将其方差贡献设为零。整体设计自由度仍须为正。

内部将权重缩放到均值 1；统一乘常数不改变估计或标准误。设计摘要保留原始权重和、范围、Kish 有效样本量 `(Σw)²/Σw²`。`weight_design_effect=n/Kish有效样本量` 只反映权重不等，不是包含分层、聚类影响的完整设计效应。无固定行数上限。

`statistic=mean` 默认计算加权均值 `Σwy/Σw`；`proportion` 要求 0/1 响应，使用同一比率估计。线性化得分为 `w(y−估计)/Σw`。

**result** 输出 estimate、设计 standard_error、95% 双侧 Wald t 区间及 design 摘要，t 自由度为设计自由度。这是估计节点，没有额外零假设或 p 值。比例区间不截断到 [0,1]，可能越界；全部响应均为 0 或均为 1 时标准误为 0，并以 `boundary_proportion` 标记，该退化区间不表示总体不确定性为零。边界比例需要专门的区间方法。

[参考：survey 抽样摘要](https://r-survey.r-forge.r-project.org/pkgdown/docs/reference/surveysummary.html)
