# Fractional Response模型

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 1–16 个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

连续比例允许 0/1 端点，总体均值须严格在 (0,1) 内。`response_link` 默认 logit，也支持 probit/cloglog。`constant=true`；500 次迭代、容差 1e-7。Bernoulli 准似然均值方程采用 HC0 得分 Sandwich 协方差及标准正态推断，不假设二项试验次数方差；真正的连续响应似然和 AIC/BIC 不适用。与 Beta 和分组二项回归不同。

$$
\hat\beta=\arg\max_\beta\sum_i[y_i\log\mu_i+(1-y_i)\log(1-\mu_i)],\quad0\le y_i\le1.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.genmod.generalized_linear_model.GLM.html)
