# 两比例 z 检验

比较两个独立总体的成功概率。

## 输入与参数

`group1`、`group2` 分别输入非空数值 `0/1` 序列，`1` 为成功，样本量可以不同。空值和其他编码会被拒绝。`null_difference` 默认 `0`，表示第一组减第二组的原假设比例差，须有限且具有可解释的概率差含义。`alternative` 默认为 `two_sided`，另可选 `greater`、`less`。

## 假设与统计量

$H_0:p_1-p_2=\delta_0$，备择为该差不等于、大于或小于 $\delta_0$。令 $x_i,n_i,\hat p_i=x_i/n_i$ 为各组成功数、试验数及样本比例：

$$
z=\frac{\hat p_1-\hat p_2-\delta_0}{SE},\qquad z\overset{H_0}{\approx}N(0,1).
$$

$\delta_0=0$ 时使用合并比例 $\hat p=(x_1+x_2)/(n_1+n_2)$：

$$
SE=\sqrt{\hat p(1-\hat p)(1/n_1+1/n_2)}.
$$

非零差值时使用 $SE=\sqrt{\hat p_1(1-\hat p_1)/n_1+\hat p_2(1-\hat p_2)/n_2}$。标准误须大于零，观测和两组应独立，成功数与失败数须足以支持正态近似。

## 输出与判读

`result`、`report` 相同。`statistic` 为 z，`estimate` 为 $\hat p_1-\hat p_2-\delta_0$，`standard_error` 为所用标准误，`sample_sizes` 为 `[n1, n2]`，自由度为空。根据所选方向解读 `p_value`；$p<\alpha$ 时拒绝 $H_0$。
