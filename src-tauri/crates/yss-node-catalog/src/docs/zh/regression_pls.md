# PLS回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

采用协方差方向和回归消去法拟合单响应 PLS1。`components=1`，须为正整数，不能超过自变量数、中心化秩及 n−1。响应/自变量均中心化，`standardize=true` 时缩放自变量。在原始单位下恢复截距。W、P、q 分别为自变量权重/载荷和响应载荷，按成分顺序保留在 `details`。没有剩余响应协方差的成分无法估计；不提供交叉验证或系数推断。

$$
\hat\beta=W(P^TW)^{-1}q.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://scikit-learn.org/stable/modules/generated/sklearn.cross_decomposition.PLSRegression.html)
