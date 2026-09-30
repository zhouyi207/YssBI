# 条件Logit回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 1–16 个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

二元响应另接对齐的类别 `groups`，最多 64 层。二元条件 Logit 按每层病例数 m_g 条件化，不估计截距。全零/全一层被删除并在 `details` 保留计数；`observations` 只统计有信息的行。自变量须有层内变化且满秩。默认 500 次迭代、容差 1e-7，正态 Wald 推断采用观测条件信息。截距被条件化，不提供绝对拟合概率、数值残差及 RSS/RMSE。此处为匹配/分层二元 Logit，不是多项选择集模型。

$$
L(\beta)=\prod_g\frac{\exp(\sum_{i\in g}y_ix_i^T\beta)}{\sum_{A\subseteq g,\ |A|=m_g}\exp(\sum_{i\in A}x_i^T\beta)},\quad m_g=\sum_{i\in g}y_i.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.discrete.conditional_models.ConditionalLogit.html)
