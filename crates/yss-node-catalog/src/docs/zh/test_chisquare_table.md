# Pearson 列联表卡方检验

直接输入已有列联表的计数，检验行列变量的独立性。

## 输入与参数

`counts` 为按行展开的非负整数频数。`rows`、`columns` 均默认为 `2`，可取 `2–1000` 的整数，输入长度必须等于两者乘积。

例如 `rows=2`、`columns=3` 时，输入顺序是 `[第1行第1列, 第1行第2列, 第1行第3列, 第2行第1列, 第2行第2列, 第2行第3列]`。数值须有限、无空值，各行列合计须大于零。

## 假设与统计量

$H_0$：行列分类独立；$H_1$：存在关联。

$$
E_{ij}=\frac{O_{i+}O_{+j}}N,\qquad
X^2=\sum_{ij}\frac{(O_{ij}-E_{ij})^2}{E_{ij}},
\qquad X^2\overset{H_0}{\approx}\chi^2_{(r-1)(c-1)}.
$$

$O_{ij}$ 为输入频数，$N$ 为总频数，$r,c$ 为行列数。使用未作连续性校正的 Pearson 统计量。频数应来自独立观测，期望频数过小时卡方近似可能不可靠。

## 输出与判读

`result` 为结构化结果。`statistic` 为 $X^2$，`degrees_of_freedom` 为 `[(r−1)(c−1)]`，`sample_sizes` 为 `[N]`，`details.minimum_expected_count` 是最小期望频数。`estimate`、`standard_error` 为空。

`p_value` 为卡方右尾概率；$p<\alpha$ 时拒绝独立性。
