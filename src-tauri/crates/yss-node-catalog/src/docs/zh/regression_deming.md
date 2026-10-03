# Deming回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`X₁, X₂, …` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

单个自变量，至少四个配对观测。正的 `variance_ratio=1` 表示响应误差方差/自变量误差方差。S 为中心化乘积和，要求两个变量均有变异且协方差非零。在固定方差比下同时考虑两变量测量误差并含截距。协方差采用逐一删除 Jackknife，系数推断采用标准正态近似；不提供似然或 AIC/BIC。

$$
\hat b=\frac{S_{yy}-\delta S_{xx}+\sqrt{(S_{yy}-\delta S_{xx})^2+4\delta S_{xy}^2}}{2S_{xy}},\quad\hat a=\bar y-\hat b\bar x.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://search.r-project.org/CRAN/refmans/deming/html/deming.html)
