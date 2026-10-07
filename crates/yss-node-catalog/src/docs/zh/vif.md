# VIF 方差膨胀因子

连接线性 `model`，诊断解释变量间的多重共线性。无配置参数；OLS、WLS、GLS 均使用原始设计矩阵，不加权或白化。

## 计算公式

$$
\mathrm{VIF}_j=\frac1{1-R_j^2},\qquad
\mathrm{Tolerance}_j=1-R_j^2.
$$

$R_j^2$ 为第 $j$ 个解释变量对其他设计列作 OLS 辅助回归的中心化决定系数。VIF 越大，表示该变量与其他变量的线性重合越强；本节点不提供 p 值。

## 输出与使用说明

`result` 为结构化结果，`test=vif`，内层 `result` 按原设计列顺序输出 `vif`、`tolerance`。截距位置保留，两字段为 Null；不输出平均 VIF。

至少要求 $n\geq k+2$，其中 $k$ 为设计列数。近乎完全共线时以 `1e99` 和零容忍度表示；辅助矩阵奇异时失败。无截距模型的 VIF 可能小于 1，不能套用通常的含截距解释。

方法细节：[VIF](https://www.statsmodels.org/dev/generated/statsmodels.stats.outliers_influence.variance_inflation_factor.html)。
