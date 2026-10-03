# 空间杜宾误差 SDEM

自变量的空间滞后与空间误差并存，不含因变量反馈；采用高斯极大似然。

## 输入与设置

- `weights`：空间权重构造节点的设计对象。
- `units`：每行的地区标识，必须唯一并与权重对象中的地区集合完全一致。按标识重排计算，观测结果恢复输入行序；文本和数字标识不互换，宽整数保持精确。
- `response`：有限数值因变量；`predictors`：至少一个有限数值自变量。输入列须等长并按当前位置逐项对应，可混合数据库与内存数列；缺失值不自动删除。
- `constant`：默认开启，自动加入截距。不要再次输入常数列。
- `max_iterations` 默认 500，至少 1；`tolerance` 默认 $10^{-7}$，范围 $[10^{-12},0.01]$。未收敛或空间参数逼近稳定区间边界时返回失败。

## 模型与推断

$$
y=X\beta+WX_*\theta+u,\quad u=\lambda Wu+\varepsilon
$$

$W$ 是输入权重；$X$ 包含所选截距，$X_*$ 仅包含原始自变量；$\varepsilon$ 为独立、同方差的高斯创新。自变量作为外生变量处理。本节点不自动修正内生性或异方差。展开后的设计必须满列秩，观测数大于回归系数数目及空间参数数目的总和。

高斯似然包含空间滤波矩阵的对数行列式。空间参数限制在 $|\rho|,|\lambda|<1/\max_i\sum_jw_{ij}$ 的稳定区间；未使用的参数视为零。该区间对非行标准化矩阵可能比完整可逆区间更保守。估计方差为创新平方和除以 $n$。协方差由完整似然的观测信息矩阵求逆得到，同时考虑回归系数、空间参数与方差的联合估计。系数检验为 $H_0:b_j=0$ 对 $H_1:b_j\ne0$，$z=\hat b_j/SE(\hat b_j)$ 渐近服从标准正态分布；报告双侧 p 值和 95% Wald 区间。

## 结果解释

`coefficients` 给出原尺度估计、标准误、统计量、p 值和置信区间；`covariance` 的行列顺序与系数表相同，不包含误差方差。滞后自变量以 `W:列名` 表示，空间参数以 `rho`、`lambda` 表示。`sigma_squared`、`log_likelihood`、`aic`、`bic`、`df_residual` 和 `iterations` 描述拟合；信息准则的参数数包含误差方差。

- `fitted` 包含观测到的 $Wy$ 项；`residuals=y-fitted`。
- `innovations=(I-\lambda W)residuals`；`reduced_fitted=(I-\rho W)^{-1}(X\hat\beta+WX_*\hat\theta)` 是不代入观测 $Wy$ 的样本内简约式预测。
- `impacts` 为各原始自变量的平均直接、间接及总效应点估计。效应矩阵为 $(I-\rho W)^{-1}(\beta_k I+\theta_k W)$，直接效应取对角均值，总效应取行和均值，间接效应为两者之差；不存在的参数取零。效应不附加未经估计的 p 值。
- `innovation_moran_i` 为创新的描述性 Moran 指数，常量创新返回空值；不套用原始观测的随机化检验。
- `unit_labels` 为权重顺序的地区标签；`observation_units` 是输入各行对应标签的从零开始索引。单截面下 `period_labels`、`observation_periods`、`unit_effects` 为空，`periods=1`。

孤立地区保留零权重行，不补充自邻接。采用密集矩阵计算，规模受执行内存预算限制。

[空间回归模型参考](https://r-spatial.github.io/spatialreg/reference/ML_models.html)
