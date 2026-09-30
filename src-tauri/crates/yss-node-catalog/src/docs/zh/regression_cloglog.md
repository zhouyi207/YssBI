# Complementary log-log模型

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 1–16 个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

二元 0/1 或 false/true 响应，须两种结果均存在。`constant=true`；500 次迭代、容差 1e-7。拟合值为概率，系数推断采用模型 Fisher 信息和标准正态参考。这种非对称链接不会自动构成生存模型；当前没有暴露量/时间偏置。

$$
\log[-\log(1-\pi_i)]=x_i^T\beta,\quad\pi_i=1-\exp[-\exp(x_i^T\beta)].
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.genmod.families.links.CLogLog.html)
