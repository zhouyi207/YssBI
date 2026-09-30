# 多维尺度分析（MDS）

采用经典度量 MDS。**MDS 输入形式** 默认 `observations`：输入 1–16 个数值 **Variable / distance column** 数列，每行为一条观测，计算欧氏距离。**标准化变量** 默认关闭，仅在此模式出现；开启时中心化并除以样本标准差，常量变量拒绝。

选择 `dissimilarity_matrix` 时，每个输入数列是距离矩阵的一列，必须方形、非负、对称，主对角线为零；最多 512 行/列。对称性和零对角线允许相对于最大距离 $10^{-12}$ 的浮点误差，不做加性距离校正。缺失值和无正变异的距离拒绝。所有输入列须对齐、关系输入共享行域。

观测数 $n\le512$；**保留维数** 默认 2，范围为 $1\le k\le\min(n-1,16)$。令 $D$ 为距离，$J=I-\mathbf1\mathbf1^T/n$，经典嵌入为

$$B=-\tfrac12J D^{\circ2}J=V\Lambda V^T,\qquad T_k=V_k\operatorname{diag}(\sqrt{\max(\lambda_j,0)}).$$

选择最大的正特征值，保留维数超过正秩时剩余坐标为零。非欧氏距离可能有负特征值，结果明确报告其数量与绝对惯性，不把负值静默解释为普通方差。

**Result** 给出保留特征值、正秩、正/负惯性、按正惯性与绝对惯性计算的两种拟合比例，以及距离压力

$$\mathrm{stress}=\sqrt{\frac{\sum_{i<j}(d_{ij}-\widehat d_{ij})^2}{\sum_{i<j}d_{ij}^2}}.$$

$\widehat d_{ij}$ 是嵌入后的欧氏距离。越小表示所选维数下距离还原越好；这不是非度量 MDS 的排序压力。本节点没有假设检验或 p 值。

**Coordinates** 是独立计算表，含 `axis1` 至 `axisK`，行顺序对应观测或距离矩阵行，可继续选列与绘图。旋转、反射或符号改变不会改变嵌入距离。

参考：[R cmdscale](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/cmdscale.html)。
