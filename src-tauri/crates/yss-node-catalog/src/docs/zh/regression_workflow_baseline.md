# 基准回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 1–16 个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

以全部已连接自变量拟合明确的 OLS 基准；`constant=true`。基准是工作流角色，此节点明确指定实际估计器并输出单个模型。系数检验采用 Student t(n−p)，AIC/BIC 计入系数和一个高斯方差参数。不比较未指定备选，不进行模型选择或因果识别。

$$
\hat\beta=\arg\min_\beta\sum_i(y_i-x_i^T\beta)^2.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.OLS.html)
