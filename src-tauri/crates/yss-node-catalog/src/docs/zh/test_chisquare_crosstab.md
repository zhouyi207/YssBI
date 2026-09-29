# 列联表卡方独立性检验

从成对分类观测构造列联表，检验两个分类变量是否独立。

## 输入与参数

`row`、`column` 输入同一批对象的分类序列，支持数值、分类、有序或二元语义。序列须非空、逐行对应、长度相等，且每个变量至少有 2 个实际类别。关系数列须来自同一行域；不接受空值，不自动合并或删除类别。无配置参数。

## 假设与统计量

$H_0$：行变量与列变量独立；$H_1$：两者有关联。

$$
E_{ij}=\frac{O_{i+}O_{+j}}N,\qquad
X^2=\sum_{i=1}^{r}\sum_{j=1}^{c}\frac{(O_{ij}-E_{ij})^2}{E_{ij}},
\qquad X^2\overset{H_0}{\approx}\chi^2_{(r-1)(c-1)}.
$$

$O_{ij}$ 是观测频数，$O_{i+},O_{+j}$ 是行、列合计，$N$ 是观测总数。使用 Pearson 统计量，不作 Yates 连续性校正。独立观测及足够大的期望频数是卡方近似的重要条件。

## 输出与判读

`result`、`report` 相同：`statistic` 为 $X^2$，`degrees_of_freedom` 为 `[(r−1)(c−1)]`，`p_value` 是卡方右尾概率，`sample_sizes` 为 `[N]`。`details.minimum_expected_count` 给出最小期望频数；`estimate`、`standard_error` 为空。

$p<\alpha$ 表示存在关联证据，不表示因果关系。稀疏的 $2\times2$ 表可改用 Fisher 精确检验。
