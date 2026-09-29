# 精确二项检验

根据成功次数检验二元试验的成功概率，使用二项分布概率。

## 输入与参数

`series` 输入数值 `0/1` 观测，`1` 为成功。至少 1 次、最多 1,000,000 次试验；不得含空值或其他编码。`null_probability` 默认为 `0.5`，范围 $[0,1]$。`alternative` 默认 `two_sided`，另可选 `greater`、`less`。

## 假设与统计量

$H_0:p=p_0$；备择分别为 $p\ne p_0$、$p>p_0$ 或 $p<p_0$。统计量是成功次数 $x$，参考模型为：

$$
X\overset{H_0}{\sim}\operatorname{Binomial}(n,p_0),\qquad
P(X=k)=\binom nk p_0^k(1-p_0)^{n-k}.
$$

双侧 p 值累加概率不大于观测结果概率的所有结果；单侧使用相应方向的二项尾概率。试验应独立，且共享同一成功概率。

## 输出与判读

`result` 和 `report` 相同。`statistic_name` 为 `successes`，`statistic` 和 `details.successes` 为成功次数；`estimate` 为 $x/n$，`sample_sizes` 为 `[n]`。`standard_error` 为空，自由度为空。`p_value` 小于 $\alpha$ 时拒绝 $H_0$。
