# 典型相关分析

输入 **X variable** 与 **Y variable** 两组逐行对应的数值数列，每组至少一列、合计最多 16 列，关系输入须共享行域。拒绝缺失值、常量列及组内秩亏。观测数 $n>p+q$，$p,q$ 分别是两组变量数。**保留维数** 默认 1，范围 $1\le k\le\min(p,q)$。

各变量按样本均值和标准差标准化。令 $R_{xx},R_{yy},R_{xy}$ 为相关块，通过白化后的交叉相关矩阵得到典型相关 $\rho_i$ 及系数 $a_i,b_i$：

$$u_i=Z_xa_i,\qquad v_i=Z_yb_i,\qquad \operatorname{Var}(u_i)=\operatorname{Var}(v_i)=1,\quad \operatorname{Corr}(u_i,v_i)=\rho_i.$$

载荷为原标准化变量与本组典型得分的相关；系数和载荷是不同概念。

每个顺序检验的 $H_0$ 是从第 $i$ 对开始的所有典型相关为零，$H_1$ 是其中至少一个非零。使用 Wilks Lambda 的 Bartlett 卡方近似：

$$\Lambda_i=\prod_{j=i}^{\min(p,q)}(1-\rho_j^2),\qquad \chi_i^2=-\left[n-1-\frac{p+q+1}{2}\right]\log\Lambda_i,\quad df_i=(p-i+1)(q-i+1).$$

该近似假定观测独立、近似联合多元正态。数值上不可区分于完全相关的根令 Lambda 为零，卡方和 p 值保留 null，不编码无穷值。

**Result** 包含全部典型相关和顺序检验、两组均值/标准差、变量×保留轴的标准化系数与载荷。检验使用全部根，不因只保留部分得分而截断。

**Scores** 将两组得分放在同一个独立计算表中，列为 `x_axis1`…`x_axisK`、`y_axis1`…`y_axisK`，保留输入行顺序；同表选出的 X/Y 轴可继续计算相关或绘图。

定义参考：[R cancor](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/cancor.html)。
