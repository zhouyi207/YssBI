# 单样本比例 z 检验

用正态近似检验二元事件的总体成功概率。

## 输入与参数

`series` 为非空的数值 `0/1` 序列，`1` 表示成功；不接受空值或其他编码。`null_probability` 默认 `0.5`，须在 $(0,1)$ 内，端点会产生零标准误。`alternative` 默认 `two_sided`，另可选 `greater`、`less`。

## 假设与统计量

$H_0:p=p_0$，备择为 $p\ne p_0$、$p>p_0$ 或 $p<p_0$。令 $x$ 为成功次数，$n$ 为试验数，$\hat p=x/n$：

$$
z=\frac{\hat p-p_0}{\sqrt{p_0(1-p_0)/n}},\qquad z\overset{H_0}{\approx}N(0,1).
$$

试验应独立且共享同一成功概率。正态近似需要足够的期望成功数和失败数；小样本可使用精确二项检验。

## 输出与判读

`result`、`report` 返回相同报告。`statistic` 为 z，`estimate` 为 $\hat p-p_0$，`standard_error` 使用原假设概率，`sample_sizes` 为 `[n]`，自由度为空。`p_value` 小于 $\alpha$ 时拒绝给定成功概率。
