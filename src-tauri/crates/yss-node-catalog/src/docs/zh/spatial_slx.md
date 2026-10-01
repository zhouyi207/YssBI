# 自变量空间滞后 SLX

将所有自变量的一阶空间滞后加入 OLS；不对截距构造滞后项。

## 输入与设置

- `weights`：空间权重构造节点的设计对象。
- `units`：每行的地区标识，必须唯一并与权重对象中的地区集合完全一致。按标识重排计算，观测结果恢复输入行序；文本和数字标识不互换，宽整数保持精确。
- `response`：有限数值因变量；`predictors`：至少一个有限数值自变量。输入列须来自同一行域，缺失值不自动删除。
- `constant`：默认开启，自动加入截距。不要再次输入常数列。

## 模型与推断

$$
y=X\beta+WX_*\theta+\varepsilon
$$

$W$ 是输入权重；$X$ 包含所选截距，$X_*$ 仅包含原始自变量；$\varepsilon$ 为独立、同方差的高斯创新。自变量作为外生变量处理。本节点不自动修正内生性或异方差。展开后的设计必须满列秩，观测数大于回归系数数目及空间参数数目的总和。

系数检验为 $H_0:b_j=0$ 对 $H_1:b_j\ne0$，统计量 $t=\hat b_j/SE(\hat b_j)$，参考 Student $t_{n-p}$ 分布；$p$ 是含截距和滞后自变量的设计列数。报告双侧 p 值及 95% 置信区间，误差方差采用 $RSS/(n-p)$。

## 结果解释

`coefficients` 给出原尺度估计、标准误、统计量、p 值和置信区间；`covariance` 的行列顺序与系数表相同，不包含误差方差。滞后自变量以 `W:列名` 表示，空间参数以 `rho`、`lambda` 表示。`sigma_squared`、`log_likelihood`、`aic`、`bic`、`df_residual` 和 `iterations` 描述拟合；信息准则的参数数包含误差方差。

- `fitted` 包含观测到的 $Wy$ 项；`residuals=y-fitted`。
- `innovations=(I-\lambda W)residuals`；`reduced_fitted=(I-\rho W)^{-1}(X\hat\beta+WX_*\hat\theta)` 是不代入观测 $Wy$ 的样本内简约式预测。
- `impacts` 为各原始自变量的平均直接、间接及总效应点估计。效应矩阵为 $(I-\rho W)^{-1}(\beta_k I+\theta_k W)$，直接效应取对角均值，总效应取行和均值，间接效应为两者之差；不存在的参数取零。效应不附加未经估计的 p 值。
- `innovation_moran_i` 为创新的描述性 Moran 指数，常量创新返回空值；不套用原始观测的随机化检验。
- `unit_labels` 为权重顺序的地区标签；`observation_units` 是输入各行对应标签的从零开始索引。单截面下 `period_labels`、`observation_periods`、`unit_effects` 为空，`periods=1`。

孤立地区保留零权重行，不补充自邻接。采用密集矩阵计算，规模受执行内存预算限制。

[空间回归模型参考](https://r-spatial.github.io/spatialreg/reference/ML_models.html)
