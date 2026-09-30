# 独立样本 t 检验

比较两个独立总体的均值。

## 输入与参数

`group1`、`group2` 分别连接两组数值序列，每组至少 2 个有限观测，允许样本量不同；空值需先处理。`equal_variance` 默认 `false`，使用 Welch 检验；设为 `true` 时采用合并方差。`alternative` 默认为 `two_sided`，另可选 `greater`、`less`，方向均按第一组减第二组解释。

## 假设与统计量

$H_0:\mu_1-\mu_2=0$；备择为该差不等于、大于或小于零。记 $n_i,\bar x_i,s_i$ 为第 $i$ 组的样本量、均值和样本标准差。

Welch 方法令 $a=s_1^2/n_1$、$b=s_2^2/n_2$：

$$
t=\frac{\bar x_1-\bar x_2}{\sqrt{a+b}},\qquad
\nu=\frac{(a+b)^2}{a^2/(n_1-1)+b^2/(n_2-1)}.
$$

等方差方法使用：

$$
s_p^2=\frac{(n_1-1)s_1^2+(n_2-1)s_2^2}{n_1+n_2-2},\qquad
SE=s_p\sqrt{1/n_1+1/n_2},\qquad \nu=n_1+n_2-2.
$$

两种方法都用 $t_\nu$ 参考分布，且要求正标准误。组间及组内观测应独立；等方差模式还假定总体方差相同。

## 输出与判读

`result` 为结构化结果：`statistic` 为 t，`estimate` 为第一组均值减第二组均值，`standard_error` 为所用标准误，`degrees_of_freedom` 为 `[ν]`，`sample_sizes` 为 `[n1, n2]`。按 `p_value` 与所选备择判断均值差异；$p<\alpha$ 时拒绝 $H_0$。
