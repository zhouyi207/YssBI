# Ridit 分析

连接有序 **Sample** 和 **Reference**，可用数值类别编码，或有显式等级顺序的 Ordinal 列。两者可以长度不同、来自独立总体，不按配对观测处理；均须非空、完整。有序输入须声明相同的类别编码顺序，数值编码按升序排列。

设参考类别占比为 $p_j$，则

$$r_j=\sum_{h<j}p_h+\tfrac12p_j,\qquad\bar r=\sum_jq_jr_j,$$

$q_j$ 为样本类别占比。平均 Ridit 估计样本观测高于参考观测的概率，并列计半个：高于 0.5 表示样本类别偏高，低于 0.5 表示偏低。参考分布的平均 Ridit 为 0.5。

`result` 含类别计数/占比、Ridit 分数、平均 Ridit，以及并列修正的 Mann–Whitney 正态检验，原假设为 $H_0:P(S>R)+\tfrac12P(S=R)=0.5$。备择默认 `two_sided`，连续性修正默认 false。小样本时正态近似可能较差，不计算精确秩检验 p 值。零假设方差为零时推断字段为 null，描述分数仍可读取。

参考：[Bross（1958），How to Use Ridit Analysis](https://doi.org/10.2307/2527727)。
