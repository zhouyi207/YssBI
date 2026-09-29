# 多组比例齐性检验

用于比较多个独立组的成功概率。

## 输入与参数

`successes_and_trials` 按 `[成功数1, 试验数1, 成功数2, 试验数2, …]` 输入至少 3 组的计数。每项须为有限非负整数，各组试验数大于零且成功数不超过试验数；不接受空值。无配置参数。

## 假设与统计量

目标原假设为 $H_0:p_1=\cdots=p_k$；备择为至少一个组比例不同。Pearson 统计量为：

$$
X^2=\sum_{ij}\frac{(O_{ij}-E_{ij})^2}{E_{ij}},\qquad
E_{ij}=\frac{O_{i+}O_{+j}}N,\qquad
X^2\overset{H_0}{\approx}\chi^2_{k-1}.
$$

$O_{ij}$ 为成功／失败列联表频数，$N$ 为总试验数。有效推断要求独立试验、正确分组及足够的期望频数。

## 输出与当前限制

`result`、`report` 相同。`statistic` 为 Pearson 统计量，`degrees_of_freedom` 为 `[k−1]`，`p_value` 为卡方右尾概率，`sample_sizes` 为总试验数。`details.overall_proportion` 为总体成功比例，`details.minimum_expected_count` 为构造表的最小期望频数；估计值和标准误字段为空。

当前节点把交错的各组成功／失败计数按两行重排，未正确保持各组边际，因此输出暂不宜作为多组比例相等的推断依据。可把数据整理为每组一行、成功和失败各一列，交给“Pearson 列联表卡方检验”。
