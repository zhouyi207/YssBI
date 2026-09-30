# 单样本等价检验（TOST）

检验总体均值是否落在事先指定的等价区间内。

## 输入与参数

`series` 至少包含 2 个有限数值，无空值且样本标准差大于零。`lower_bound` 默认为 `-0.5`，`upper_bound` 默认为 `0.5`；两者须有限，且下界严格小于上界。界限是均值的绝对界限，使用输入数据的单位。本节点没有 `alternative` 参数。

## 假设与统计量

令下界、上界为 $L,U$。$H_0:\mu\le L$ 或 $\mu\ge U$；$H_1:L<\mu<U$。对 $SE=s/\sqrt n$ 计算：

$$
t_L=\frac{\bar x-L}{SE},\qquad t_U=\frac{\bar x-U}{SE},\qquad \nu=n-1.
$$

下界检验 $H_{0L}:\mu\le L$ 对 $H_{1L}:\mu>L$，上界检验 $H_{0U}:\mu\ge U$ 对 $H_{1U}:\mu<U$：

$$
p_L=P(t_\nu\ge t_L),\qquad p_U=P(t_\nu\le t_U),\qquad
p_{\mathrm{TOST}}=\max(p_L,p_U).
$$

$n,\bar x,s$ 分别为样本量、均值和样本标准差。观测应独立，小样本时近似正态；界限应在查看结果前确定。

## 输出与判读

`result` 为结构化结果。`statistic` 为 $t_L$，`details.t_upper_bound` 为 $t_U$；`details.p_lower_bound`、`details.p_upper_bound` 保存两个单侧 p 值。`p_value` 是二者最大值，`estimate` 是均值，`standard_error` 是 $SE$，自由度为 `[n−1]`，样本量为 `[n]`。

只有两个单侧检验都在 $\alpha$ 水平拒绝时才支持等价。普通差异检验不显著不能代替等价结论。
