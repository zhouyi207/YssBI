# 曲线回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`X₁, X₂, …` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

连接单个 `X`。选择多项式（默认 `degree=2`，1–8）、对数、倒数、指数或幂函数，均包含截距/幅度。对数/幂要求 x>0，倒数要求 x≠0，指数/幂要求 y>0。指数/幂拟合响应的对数，`amplitude` 为指数化的截距并转换 Delta 协方差，拟合曲线在对数正态误差下表示条件中位数，AIC/BIC 包含响应 Jacobian。其余模型拟合原始响应；系数推断采用残差自由度的 Student t 分布。

$$
y=a+\sum_{j=1}^d b_jx^j,\quad y=a+b\ln x,\quad y=a+b/x,\quad y=ae^{bx},\quad y=ax^b.
$$

正的幅度参数区间从对数截距尺度转换，其边界零假设统计量/p 值为 null。

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.OLS.html)
