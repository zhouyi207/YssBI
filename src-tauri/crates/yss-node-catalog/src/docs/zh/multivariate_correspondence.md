# 对应分析

输入二维非负频数/权重表的各个 **Count column** 数列；至少 2 行、2 列，各列必须对齐。行与列代表类别，不代表原始个体观测。不接受负值、缺失值及总量为零的行或列。**保留维数** 默认 1，须不超过 $\min(r-1,c-1)$。

令 $N$ 为总权重，$P$ 为除以 $N$ 的相对频数矩阵，$a,b$ 为行、列质量，$D_a,D_b$ 为其对角矩阵：

$$S=D_a^{-1/2}(P-ab^T)D_b^{-1/2}=U\Sigma V^T,\quad F=D_a^{-1/2}U\Sigma,\quad G=D_b^{-1/2}V\Sigma.$$

$F,G$ 是行与列的主坐标，轴惯性为 $\lambda_j=\sigma_j^2$，总惯性 $I=\sum_j\lambda_j$，描述性 Pearson 量为 $NI$。

**Result** 返回质量、特征值、惯性比例、保留比例和 `chi_square`。本节点接受非整数权重，不提供 p 值；不能把任意权重的 `chi_square` 自动当作频数表独立性检验。独立表的惯性与坐标为零，未定义的惯性比例保留 null。

**Row coordinates** 和 **Column coordinates** 分别是两张独立数值表，列为 `axis1` 至 `axisK`，行顺序对应输入表的行类别或输入列顺序。可连接选列和绘图；两个类别域不能按等长数列直接视为对齐。跨行/列类别的欧氏距离通常不能直接解释为类别距离。

参考：[R MASS corresp](https://stat.ethz.ch/R-manual/R-devel/library/MASS/html/corresp.html)。
