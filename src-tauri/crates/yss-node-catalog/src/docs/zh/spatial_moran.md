# 莫兰指数

计算全局 Moran $I$，衡量地区属性与相邻地区属性的空间自相关。

## 输入与参数

`weights` 接收空间权重设计对象；`units` 必须唯一且与权重地区集合一致；`response` 为非恒定有限数值属性。两列必须对齐，缺失值不自动删除。按精确地区标识匹配权重，保留零行孤立地区，至少两行数据。`spatial_permutations` 默认 999，非负整数，0 关闭置换推断；`seed` 默认 42，为非负整数。同一输入、顺序和种子可复现。

## 统计量与检验

$$
I=\frac{n}{S_0}\frac{z^TWz}{z^Tz},\qquad z_i=y_i-\bar y,\qquad S_0=\sum_{ij}w_{ij},\qquad E(I)=-\frac1{n-1}.
$$

$H_0$：属性对地区的分配不存在空间关联；$H_1$：存在正向或负向关联。$I$ 相对于期望更大表示相似值聚集，更小表示相邻值倾向不同。其取值不保证在 $[-1,1]$ 内。

正态假设下，令 $S_1=\frac12\sum_{ij}(w_{ij}+w_{ji})^2$、$S_2=\sum_i(\sum_jw_{ij}+\sum_jw_{ji})^2$：

$$
V_N(I)=\frac{n^2S_1-nS_2+3S_0^2}{(n^2-1)S_0^2}-E(I)^2.
$$

随机化假设固定属性值，仅随机置换位置。对 $n>3$，令 $b_2=n\sum_i z_i^4/(\sum_i z_i^2)^2$：

$$
V_R(I)=\frac{n[(n^2-3n+3)S_1-nS_2+3S_0^2]-b_2[(n^2-n)S_1-2nS_2+6S_0^2]}{(n-1)(n-2)(n-3)S_0^2}-E(I)^2.
$$

两种解析检验均用 $z=(I-E(I))/\sqrt{V(I)}$ 的标准正态近似，报告双侧 p 值；样本较小或方差退化时不提供相应推断。方差不大于 $10^{-14}$ 视为数值退化。

置换检验固定 $W$，在全部地区间打乱属性，以 $|I-E(I)|$ 判定双侧极端性。若 $B$ 次置换中有 $b$ 次至少同样极端，返回 $p=(b+1)/(B+1)$；这与取置换分布较小单尾概率的约定不同。该 p 值没有参数自由度。

## 输出

`statistic`、`expected` 是 $I$ 及其零假设期望；`normal_variance`、`normal_z`、`normal_p_value` 给出正态假设下的方差、z 统计量和 p 值，对应的 `randomization_*` 字段给出随机化假设下的解析结果。`permutation_p_value` 为置换 p 值，关闭置换时为空；`observations`、`permutations` 和 `seed` 记录运行设置。全连接等退化权重下解析推断可为空，置换 p 值仍可为 1。

本节点用于原始属性的全局分析，不对回归残差提供修正后的检验，也不进行局部 LISA 检验。

[Moran 的假设与推断](https://pysal.org/esda/stable/user-guide/global_morans_i.html)
