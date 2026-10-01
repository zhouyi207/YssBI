# 多分类Logit

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

响应可为文本/标识符/数值编码类别。观测类别按首次出现排列，`categories[0]` 为参照。至少两个观测类别，n 须大于参数数目。`constant=true`；500 次迭代、容差 1e-7。`class[c].xj` 索引非参照类别 c 和自变量 j。`probabilities` 行对应观测，列对应 `categories`；`fitted_categories` 恢复原标签。数值拟合/残差数组为空，RSS/RMSE 为 null。Wald 推断采用观测信息和标准正态参考分布；分离/信息矩阵奇异时失败。

$$
P(Y=0\mid x)=\frac1{1+\sum_{c=1}^{K-1}e^{x^T\beta_c}},\quad P(Y=c\mid x)=\frac{e^{x^T\beta_c}}{1+\sum_{h=1}^{K-1}e^{x^T\beta_h}}.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.discrete.discrete_model.MNLogit.html)
