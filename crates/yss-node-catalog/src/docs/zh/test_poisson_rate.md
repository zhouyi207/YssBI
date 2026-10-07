# Poisson 率检验

检验等暴露量观测的平均事件发生率。

## 输入与参数

`series` 为非空的非负整数计数序列；每个观测应具有相同暴露量，不得含空值。`null_rate` 默认为 `1`，是原假设下每次观测的平均计数，须有限且非负。`alternative` 默认 `two_sided`，另可选 `greater`、`less`。

总事件数最多 1,000,000，原假设总期望计数最多 500,000。本节点没有不等暴露量输入。

## 假设与统计量

$H_0:\lambda=\lambda_0$；备择为 $\lambda\ne\lambda_0$、$\lambda>\lambda_0$ 或 $\lambda<\lambda_0$。令 $S=\sum_i x_i$、$m=n\lambda_0$：

$$
S\overset{H_0}{\sim}\operatorname{Poisson}(m),\qquad
P(S=k)=e^{-m}\frac{m^k}{k!}.
$$

双侧 p 值按不大于观测概率的 Poisson 概率质量求和，单侧按指定方向计算。观测应独立且符合 Poisson 计数模型；过度离散会影响推断。

## 输出与判读

`result` 为结构化结果。`estimate` 为 $S/n$，`standard_error` 为 $\sqrt{S/n}/\sqrt n$，`sample_sizes` 为 `[n]`。`details.event_count` 与 `details.expected_event_count` 分别是 $S$、$m$。

`statistic_name` 为 `count_score`，其值为 $(S-m)/\max(\sqrt m,1)$；`p_value` 来自 Poisson 概率，而非该分数的正态近似。自由度为空。$p<\alpha$ 时拒绝给定发生率。
