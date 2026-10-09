# Mann–Kendall 趋势检验

检验按顺序排列的观测是否具有单调趋势。

## 输入与参数

`series` 至少包含 3 个有限数值，无空值。序列当前顺序就是分析顺序；节点不接收时间列，不自动排序或作季节调整。`alternative` 默认 `two_sided`，另可选 `greater`（上升趋势）和 `less`（下降趋势）。

## 假设与统计量

$H_0$：没有单调趋势；$H_1$：存在所选方向的单调趋势。令 $n$ 为观测数：

$$
S=\sum_{i<j}\operatorname{sgn}(x_j-x_i),\qquad
Z=\begin{cases}
(S-1)/\sqrt V,&S>0,\\
0,&S=0,\\
(S+1)/\sqrt V,&S<0.
\end{cases}
$$

无并列值时 $V=n(n-1)(2n+5)/18$，使用 $Z\overset{H_0}{\approx}N(0,1)$。有并列值时从该方差减去 $\sum_g t_g(t_g-1)(2t_g+5)/18$，$t_g$ 为各并列组大小，采用 [Mann–Kendall 并列修正](https://search.r-project.org/CRAN/refmans/trend/html/mk.test.html)。分数为零时保留 $Z=0$，包括所有观测相同的情况。

观测应独立；节点不校正自相关或季节性。正 $Z$ 对应上升，负 $Z$ 对应下降。

## 输出与判读

`result` 为结构化结果。`statistic_name` 为 `S_corrected_z`，`statistic` 为 $Z$，`details.s_statistic` 为 $S$；`details.kendall_tau` 为 $S/\binom n2$，不是并列修正的 tau-b。`p_value` 使用所选方向，`sample_sizes` 为 `[n]`；自由度为空，估计值和标准误为空。

在参考条件成立时，$p<\alpha$ 支持单调趋势；节点不估计变化斜率。
