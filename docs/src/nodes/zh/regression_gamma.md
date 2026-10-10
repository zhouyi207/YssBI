# Gamma回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`X₁, X₂, …` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

响应须严格为正，固定对数链接。`constant=true`；500 次迭代、容差 1e-7。Pearson 离散度 φ 按 n−p 自由度估计，p 为均值系数数目；拟合值是条件均值。`details` 保留离散度/偏差。推断采用按离散度缩放的模型 Fisher 协方差及标准正态参考；当前无离散度回归或额外链接选择。

$$
\log\mu_i=x_i^T\beta,\quad\operatorname{Var}(Y_i\mid x_i)=\phi \mu_i^2.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.genmod.families.family.Gamma.html)
