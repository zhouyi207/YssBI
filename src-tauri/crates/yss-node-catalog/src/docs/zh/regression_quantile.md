# 分位数回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

`quantile=0.5` 为条件中位数，须严格在 0 与 1 之间；`constant=true`。IRLS 使用小的残差下限，5000 次迭代、容差 1e-7，`details.check_loss` 保留原始检验损失。IID 协方差为 τ(1−τ)(XᵀX)⁻¹/f̂(0)²，使用高斯残差密度核和 Silverman 带宽；检验/95% 区间采用标准正态近似。密度退化时推断为 null。当前无聚类/异方差分位数协方差，RSS 指标不能替代检验损失。

$$
\hat\beta_\tau=\arg\min_\beta\sum_i\rho_\tau(y_i-x_i^T\beta),\quad\rho_\tau(u)=u[\tau-\mathbf1(u<0)].
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.regression.quantile_regression.QuantReg.fit.html)
