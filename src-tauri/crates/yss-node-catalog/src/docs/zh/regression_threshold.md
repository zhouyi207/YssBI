# 门槛回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 1–16 个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

另接对齐的 `threshold_variable` q。单个未知门槛，两侧系数分别估计；`constant=true`。`trimming=0.15`（0.05–0.45），两侧还各须 p+2 行。搜索满足样本要求的不同观测候选，`max_candidates=100`（1–200）对更多排序候选均匀抽取。最小 RSS 胜出，并列选最先者。`below.*`/`above.*` 分别表示 q≤c/q>c。明细保留门槛、两侧人数及搜索规模。合并方差的 t(n−2p) 推断把选定门槛视为固定，不提供门槛显著性/搜索调整检验；无面板或多门槛变体。

$$
(\hat c,\hat\beta_1,\hat\beta_2)=\arg\min_{c,\beta_1,\beta_2}\sum_i[y_i-\mathbf1(q_i\le c)x_i^T\beta_1-\mathbf1(q_i>c)x_i^T\beta_2]^2.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.ssc.wisc.edu/~bhansen/papers/ecnmt_00.pdf)
