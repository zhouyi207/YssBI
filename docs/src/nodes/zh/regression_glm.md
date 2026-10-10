# GLM广义线性模型

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`X₁, X₂, …` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

`glm_family` 默认 poisson。Gaussian 支持 `gaussian_link=identity`（默认）或 log；Binomial 支持 `binomial_link=logit`（默认）、probit 或 cloglog；Poisson/Gamma/逆高斯固定 log。对应方差函数分别为 1、μ(1−μ)、μ、μ²、μ³。Binomial 要求 0/1，Poisson 要求非负整数且均值为正，Gamma/逆高斯要求正响应。`constant=true`；500 次迭代、容差 1e-7。Gaussian/Gamma/逆高斯按 n−p 自由度估计 Pearson 离散度，Binomial/Poisson 使用 1。Fisher 协方差按离散度缩放；Gaussian 采用 Student t(n−p)，其余采用标准正态。`details` 含分布/链接/离散度/偏差，当前无其他链接或偏置。

$$
g(\mu_i)=x_i^T\beta,\quad\operatorname{Var}(Y_i\mid x_i)=\phi V(\mu_i).
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/glm.html)
