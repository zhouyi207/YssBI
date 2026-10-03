# 中介作用（单一观测中介）

连接 response Y、predictor X、mediator M，可追加连续协变量 C。拟合两个含截距的 OLS 方程：

$$M=i_M+aX_c+g^TC_c+e_M,\qquad Y=i_Y+c'X_c+bM_c+h^TC_c+e_Y.$$

下标 c 表示减去样本均值。`input_centers` 按 X、M、协变量顺序报告中心；系数仍使用原始单位。直接效应为 c′，间接效应为 ab，总效应为 c′+ab。协变量进入两个方程。

所有列须对齐、有限且无缺失，不自动删除行。每个方程必须满秩，观测数须大于该方程参数数目。

`replications` 默认 1000，`seed` 默认 42（非负整数）。重采样次数为 0 时只报告效应点估计，否则至少 2 次。每次对整行有放回抽样并重新拟合两个方程，保持原样本中心和探查坐标；任何一次拟合失败均停止，不静默丢弃。次数越多，百分位区间通常越稳定。没有固定行数上限。

**result** 包含样本数、输入名称、两个方程的系数/协方差/拟合指标，以及 `details.direct`、`details.effects` 的直接、间接和总效应。方程系数提供经典同方差 OLS 标准误、双侧零系数 t 检验及 95% 区间，自由度为 n−k。效应推断使用独立观测的配对行 Bootstrap：标准误为重采样估计的样本标准差，95% 区间使用 2.5% 和 97.5% 的 Type-7 百分位；不提供正态近似 p 值。间接效应区间不含 0 对应双侧零间接效应的区间证据。

**observations** 分页表保留所有行：`observation, response, fitted, residual, mediator, mediator_fitted, mediator_residual`，行号从 1 开始。

只支持单一连续观测中介和连续线性结果，不涵盖串联/多中介、潜变量、聚类重采样或非线性链接。中介的因果解释还依赖时间顺序、混杂控制及正确模型等设计假设。

[参考：lavaan 中介模型与效应定义](https://lavaan.ugent.be/tutorial/mediation.html)
