# 单样本 Wilcoxon 符号秩检验

检验样本分布是否以指定位置为对称中心。

## 输入与参数

`series` 输入有限数值序列，不接受空值。`null_median` 默认为 `0`，须有限。先计算 $d_i=x_i-m_0$，其中 $m_0$ 是 `null_median`；零差值不参与排序，至少需要 2 个非零差值。

`alternative` 默认 `two_sided`，另可选 `greater`、`less`。

## 假设与统计量

$H_0$：总体以 $m_0$ 为对称中心；备择为中心不等于、大于或小于 $m_0$。观测应独立；这不是仅凭中位数相等、无需对称性假设的检验。

对非零 $|d_i|$ 排秩，并列值取平均秩 $r_i$。令 $W^+$ 为正差值秩和：

$$
W^+=\sum_{d_i>0}r_i,\qquad
Z=\frac{W^+-\frac12\sum_i r_i}{\sqrt{\frac14\sum_i r_i^2}}.
$$

非零差值数 $n\le20$ 时使用符号枚举精确 p 值；更多观测使用 $N(0,1)$ 近似，不作连续性校正。双侧检验比较相对秩和中心的偏离，单侧按指定方向。

## 输出与判读

`result`、`report` 相同。`statistic_name` 为 `signed_rank`，`statistic` 存放标准化的 $Z$；原始 $W^+$ 在 `details.positive_rank_sum`。`details.exact_p_value_used` 为 `1` 时使用精确法，为 `0` 时使用正态近似。`sample_sizes` 为去除零差值后的 `[n]`，自由度为空，估计值和标准误为空。

`p_value` 小于 $\alpha$ 时拒绝指定对称中心；大量零差值会降低实际使用的样本量。
