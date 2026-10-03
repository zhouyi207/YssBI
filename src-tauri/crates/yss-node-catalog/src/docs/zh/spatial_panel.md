# 空间面板模型（实体固定效应）

在平衡面板中，使用正交时间对比消去每个地区的固定效应，再估计高斯空间滞后或空间误差模型。地区权重在所有时期保持不变。

## 输入与参数

- `weights`：每个地区一行的空间权重设计对象；不要将重复的面板行直接用于权重构造。
- `units`、`periods`：地区和时期标识。每个地区—时期组合恰好一行，各时期必须覆盖权重的全部地区。无需预先排序，时期标签按首次出现排序。
- `response`、一个或多个 `predictors`：等长且按当前位置逐项对应的有限数值列。缺失、不平衡或重复组合均拒绝；时间不变或共线自变量在固定效应消去后不可识别，须先移除。
- `spatial_panel_model`：`slm`（默认）或 `sem`。
- `max_iterations` 默认 500，至少 1；`tolerance` 默认 $10^{-7}$，范围 $[10^{-12},0.01]$。至少两个地区和两个时期；不另加全局截距。

## 模型与推断

SLM 为 $y_t=\rho Wy_t+X_t\beta+\alpha+\varepsilon_t$；SEM 为 $y_t=X_t\beta+\alpha+u_t$、$u_t=\lambda Wu_t+\varepsilon_t$。$\alpha$ 是不随时间变化的地区固定效应，创新独立、同方差且服从高斯分布。

以满足 $H\mathbf1=0$、$HH^T=I_{T-1}$ 的正交 Helmert 对比变换时间轴。变换后有效观测数为 $N(T-1)$，对数行列式贡献为 $(T-1)\log|I-\rho W|$ 或 $(T-1)\log|I-\lambda W|$。报告的是此变换似然，信息准则不把已消去的地区效应作为自由参数；不能与使用 $NT$ 个去均值观测的未校正似然直接比较。

空间参数的稳定区间为绝对值小于 $1/\max_i\sum_jw_{ij}$，非标准化权重下可能较保守。系数和空间参数的协方差由联合观测信息矩阵得到。检验 $H_0:b_j=0$ 对 $H_1:b_j\ne0$，使用渐近标准正态 $z=\hat b_j/SE(\hat b_j)$，报告双侧 p 值和 95% Wald 区间。有效观测数须大于回归及空间参数总数；不报告地区效应的单独显著性。

## 输出与范围

`coefficients`、`covariance`、`rho` 或 `lambda`、`sigma_squared`、`log_likelihood`、`aic`、`bic`、`df_residual`、`iterations` 为变换模型结果。`observations=NT`，`estimation_observations=N(T-1)`，`periods=T`。

`unit_labels` 按权重顺序、`period_labels` 按首次出现排序；`observation_units`、`observation_periods` 是对应标签的从零开始索引，保持输入行序。`unit_effects` 按权重顺序恢复 $\hat\alpha_i$。`fitted` 包含地区效应与观测空间滞后，`residuals=y-fitted`，`innovations` 是空间误差滤波后的残差，`reduced_fitted` 通过空间反馈逆矩阵计算并包含地区效应；这四个数组均恢复输入行序。

`impacts` 给出平均直接、间接和总效应点估计；SLM 采用 $(I-\rho W)^{-1}\beta_k I$，SEM 的直接和总效应为 $\beta_k$、间接效应为零。`innovation_moran_i` 按时期给出描述性指数，不产生残差检验 p 值。

支持静态实体固定效应；不含随机效应、双向固定效应、滞后因变量的时间动态或随时间改变的权重。计算受执行预算控制，失败或未收敛不返回部分拟合。

[空间模型定义](https://r-spatial.github.io/spatialreg/reference/ML_models.html)
