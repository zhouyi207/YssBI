# Robust回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 1–16 个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

`robust_loss` 选择 Huber（默认）或 Tukey 双权。正的 `tuning` 默认 1.345，Tukey 常用 4.685。尺度 s 为残差相对零的绝对偏差中位数除以 0.67448975，在 IRLS 中更新。`constant=true`；最大迭代 500 次、容差 1e-7。此处为 M 估计，不是为 OLS 选择 HC/聚类标准误。推断采用 H1 协方差与标准正态参考分布，不报告稳健模型似然或 AIC/BIC。

$$
\hat\beta=\arg\min_\beta\sum_i\rho((y_i-x_i^T\beta)/s).
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.robust.robust_linear_model.RLM.html)
