# 非线性回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`X₁, X₂, …` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

单个 `X`。模型为指数 b1·exp(b2·x)（默认）、Logistic 生长 b1/[1+exp(−b2·(x−b3))]、Michaelis–Menten b1·x/(b2+x) 或 Gompertz b1·exp[−exp(−b2·(x−b3))]。`initial_values` 按 b1、b2……排列，留空自动初始化。采用局部阻尼最小二乘，最大迭代 500 次、容差 1e-7。J 为拟合 Jacobian，p 为参数数目，须 J 满秩且 n>p。推断采用 n−p 自由度的局部 Student t 近似，不保证全局最优。

$$
\hat\theta=\arg\min_\theta\sum_i[y_i-f(x_i,\theta)]^2,\quad\widehat{\operatorname{Cov}}(\hat\theta)=\frac{RSS}{n-p}(J^TJ)^{-1}.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.least_squares.html)
