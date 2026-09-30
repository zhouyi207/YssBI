# 配对 Wilcoxon 符号秩检验

检验配对差值是否围绕零对称分布。

## 输入与参数

连接 `before`、`after` 数值序列，按同一对象的顺序配对，长度须相等；关系数列须共享行域。差值定义为 $d_i=before_i-after_i$。空值和非有限值会被拒绝，零差值不参与排序，至少需要 2 个非零差值。

`alternative` 默认 `two_sided`，另可选 `greater`、`less`，方向按 `before−after` 判断。

## 假设与统计量

$H_0$：差值分布以零为对称中心；备择为中心不等于、大于或小于零。不同对象应独立；位置解释需要差值分布具有对称性。

对非零 $|d_i|$ 排秩，并列值取平均秩 $r_i$。令 $W^+$ 为正差值秩和：

$$
W^+=\sum_{d_i>0}r_i,\qquad
Z=\frac{W^+-\frac12\sum_i r_i}{\sqrt{\frac14\sum_i r_i^2}}.
$$

非零差值数 $n\le20$ 时使用符号枚举精确 p 值；更多观测使用 $N(0,1)$ 近似，不作连续性校正。双侧检验比较相对秩和中心的偏离，单侧按指定方向。

## 输出与判读

`result` 为结构化结果。`statistic_name` 为 `signed_rank`，`statistic` 存放标准化的 $Z$；原始 $W^+$ 在 `details.positive_rank_sum`。`details.exact_p_value_used` 为 `1` 时使用精确法，为 `0` 时使用正态近似。`sample_sizes` 为去除零差值后的 `[n]`，自由度为空，估计值和标准误为空。

`p_value` 小于 $\alpha$ 时拒绝指定对称中心；大量零差值会降低实际使用的样本量。
