# Kappa 一致性

每个 **Rater** 输入为同一批对象的对齐分类列，在详细面板添加评定者，最多 64 列。支持数值、分类、有序、二元、文本和标识符编码；缺失评分会报错。原始类别值和宽整数标签被保留，最多支持 256 类。

Kappa 定义默认 `cohen`，必须恰好两位评定者。评定者可互换、每个对象都有相同数量完整评分时，可选 `fleiss`。Cohen 按每位评定者的边际分布估计偶然一致，Fleiss 使用合并边际分布。

$$\kappa=\frac{P_o-P_e}{1-P_e}.$$

Cohen 类别权重默认 `none`；`linear`、`quadratic` 的一致性权重分别为 $1-|i-j|/(K-1)$、$1-(i-j)^2/(K-1)^2$。加权要求升序数值类别，或相同的显式有序等级表。Ordinal 保留未出现的等级；数值编码按有序、等间距的类别位置处理。不会自动推断普通文本的加权顺序。Fleiss 不加权。

唯一 `result` 含 Kappa、实际/偶然一致率、类别标签和各评定者计数；Cohen 还输出按该类别顺序排列的列联矩阵。默认 0.95 置信水平的双侧正态区间使用渐近标准误：Cohen 为多项分布 delta 方差，Fleiss 为对象层级 delta 方差。$H_0:\kappa=0$ 的正态检验另用零假设方差。正态区间可能超出系数理论范围。偶然一致率为 1 时会报错；零方差下无法推断的字段为 null。负 Kappa 保留。

参考：[statsmodels Cohen Kappa](https://www.statsmodels.org/stable/generated/statsmodels.stats.inter_rater.cohens_kappa.html)、[R Fleiss Kappa](https://search.r-project.org/CRAN/refmans/irr/html/kappam.fleiss.html)。
