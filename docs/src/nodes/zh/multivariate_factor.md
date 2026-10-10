# 探索性因子分析

输入 至少三个逐行对齐的数值 **Variable**，数据库与内存数列可混合，须等长并按当前位置逐项对应，拒绝缺失值和常量列。使用相关矩阵的迭代主轴因子法，初始共同度为平方复相关；不采用 PCA 提取或最大似然。观测数须大于变量数 $p$，相关矩阵须正定。

**保留维数** 默认 1，即因子数 $k$；须满足 $k<p$ 且 $[(p-k)^2-(p+k)]/2\ge0$ 的可识别性约束。**最大迭代次数** 默认 500（正整数），**收敛容差** 默认 $10^{-6}$，须在 $(0,0.1]$ 内；主轴迭代与旋转使用同一上限。共同度达到或超过 1 的 Heywood 解及未收敛会失败。

对角线替换为共同度的相关矩阵，其前 $k$ 个非负特征根与向量给出载荷 $L$，并更新 $h_i^2=\sum_jL_{ij}^2$。**因子旋转** 默认 `varimax`，可选 `none`；采用正交 varimax，不做 Kaiser 归一化：

$$Q(L)=\sum_j\left[\sum_iL_{ij}^4-\frac{1}{p}\left(\sum_iL_{ij}^2\right)^2\right].$$

**Result** 返回载荷、共同度、特殊方差 $1-h_i^2$、各因子方差占比、迭代次数与 KMO。KMO 比较相关平方和与偏相关平方和，分母为零时为 null。Bartlett 球形性检验的 $H_0:R=I$，$H_1:R\ne I$：

$$\chi^2=-\left[n-1-\frac{2p+5}{6}\right]\log|R|\ \approx\chi^2_{p(p-1)/2}.$$

其中 $n$ 为观测数。较小的 Bartlett p 值支持存在相关性，不证明因子模型适配良好；该节点不提供全模型拟合检验。

**Scores** 按回归法 $\widehat F=Z R^{-1}L$ 计算，$Z$ 为按样本标准差标准化的数据。它是独立计算表，列为 `axis1` 至 `axisK`，保持原行顺序，可通过“选列”继续分析。旋转或符号改变会改变得分轴的表示，不改变载荷的共同协方差。

方法参考：[statsmodels Factor](https://www.statsmodels.org/stable/generated/statsmodels.multivariate.factor.Factor.html)。
