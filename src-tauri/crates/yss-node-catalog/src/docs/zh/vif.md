# VIF 方差膨胀因子

连接已拟合线性 `model`，衡量某个解释变量与其他解释变量的线性重合程度。节点没有配置参数。OLS、WLS 和 GLS 均检查**原始设计矩阵**，不使用 WLS 权重或 GLS 白化设计。

## 计算公式

对原设计中的每个非常数列 $x_j$，用其他设计列 $X_{-j}$ 对它作 OLS 辅助回归，得到预测值 $\hat x_{ij}$：

$$
R_j^2=1-\frac{\sum_{i=1}^n(x_{ij}-\hat x_{ij})^2}
{\sum_{i=1}^n(x_{ij}-\bar x_j)^2},\qquad
\mathrm{VIF}_j=\frac1{1-R_j^2},\qquad
\mathrm{Tolerance}_j=1-R_j^2.
$$

$n$ 是拟合样本数，$\bar x_j$ 是该列均值。含截距、满秩的经典同方差 OLS 中，

$$
\operatorname{Var}(\hat\beta_j\mid X)
=\frac{\sigma^2}{\sum_i(x_{ij}-\bar x_j)^2}\,\mathrm{VIF}_j.
$$

因此 VIF 衡量由线性共线性带来的系数方差放大；标准误的放大因子为 $\sqrt{\mathrm{VIF}_j}$。它是设计诊断，**没有原假设、备择假设、参考检验分布或 p 值**。背景见 [statsmodels VIF](https://www.statsmodels.org/dev/generated/statsmodels.stats.outliers_influence.variance_inflation_factor.html)。

## 输出与示例

`result`、`report` 相同，`test=vif` 的内层 `result` 是按原设计列顺序排列的数组，每项含 `vif`、`tolerance`。已配置截距仍占据相应位置，两字段为 Null；节点不额外输出平均 VIF。

例如，$R_j^2=0.8$ 对应 VIF 为 5、容忍度为 0.2，经典 OLS 标准误相对于同等变异下无共线性的基准放大约 $\sqrt5$ 倍。5 或 10 是常见经验关注阈值，不是显著性临界值，也不是必须删变量的规则；还需考虑估计目标、样本量和变量含义。

## 条件与边界

至少要求 $n\geq k+2$（$k$ 为设计列数）。辅助矩阵奇异或非有限计算会失败。$R_j^2\geq1-10^{-10}$ 时以 `1e99` 表示极大的 VIF，容忍度为 0。非常数目标列若数值上几乎没有变动，当前按 $R_j^2=0$ 处理，不能因此解释为良好设计。

若原模型不含截距，辅助回归也不补截距，但分母仍中心化；此时 $R_j^2$ 可能为负，VIF 可能小于 1，不能套用通常的含截距 VIF 解释。WLS/GLS 输出只描述原始自变量相关结构，不是其加权或广义最小二乘系数方差的完整分解。
