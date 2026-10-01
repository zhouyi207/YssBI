# 组内一致性 rwg

输入一个组的对齐 Numeric **Scale item** 列，每行为一位评分人，每列为一个条目。至少两位评分人；多个组请分别准备各组样本。

零假设分布默认 `uniform`，评分选项数默认 5。分数须为 1 到 A 的整数，期望方差为 $\sigma_E^2=(A^2-1)/12$。也可选择 `specified_variance`，指定与条目同一尺度上的有限正期望方差。

对于样本方差为 $s_j^2$ 的单条目，

$$r_{wg,j}=1-s_j^2/\sigma_E^2.$$

`items` 返回每条目的原始值、截为非负的 rwg 和观测方差。J 个条目采用平均观测方差，并令 $q=\min(1,\bar s^2/\sigma_E^2)$：

$$r_{wg(J)}=\frac{J(1-q)}{J(1-q)+q}.$$

`result` 含零假设定义、期望/平均观测方差、条目明细、汇总 `rwg_j` 和 `variance_truncated`。聚合时超过零假设的方差被截断，得到零一致性；条目的负原始值保留。这是描述性一致性指标，不输出 p 值或置信区间。解释应依据合理的零假设分布和评分尺度，不自动采用统一的聚合阈值。

参考：[multilevel rwg(J)](https://search.r-project.org/CRAN/refmans/multilevel/html/rwg.j.html)。
