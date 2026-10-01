# 非线性回归（自定义公式）

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

`formula` 默认 `b1 + b2*x1`；x1、x2……按自变量端口排列，b1、b2……按 `initial_values` 排列（默认 `[0,1]`）。支持 `+ - * / ^`、括号和 `exp ln sqrt abs sin cos min max`。非空 `lower_bounds`/`upper_bounds` 逐参数填写有限边界，须下界<上界且初始值在界内，留空表示该侧无约束。到达活动边界后继续优化自由参数。默认 500 次迭代、容差 1e-7，要求 n>p 且 J 满秩。内点推断采用 Student t(n−p)；活动边界处协方差/推断为 null。不可微点及局部极值可能阻止拟合。

$$
\hat\theta=\arg\min_\theta\sum_i[y_i-f(x_i,\theta)]^2,\quad\widehat{\operatorname{Cov}}(\hat\theta)=\frac{RSS}{n-p}(J^TJ)^{-1}.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.least_squares.html)
