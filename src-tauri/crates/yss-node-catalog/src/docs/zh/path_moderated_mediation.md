# 有调节的中介作用（单阶段调节）

连接 response Y、predictor X、mediator M、moderator W，可追加连续协变量。所有解释变量按原样本均值中心化，`input_centers` 依次对应 X、M、W、协变量。两个方程均含截距、W 主效应和所有协变量。

`stage=first`（默认）在中介方程中加入 Xc×Wc：M 的 X 系数为 a，交互系数为 aW；结果方程为 Y~Xc+Mc+Wc+协变量。条件间接效应为 `(a+aW×Wc)×b`，有调节中介指数为 aW×b，其中 b 为结果方程的 M 系数。

`stage=second` 在结果方程加入 Mc×Wc，交互系数为 bW；中介方程为 M~Xc+Wc+协变量。条件间接效应为 `a×(b+bW×Wc)`，指数为 a×bW。直接效应 c′不随 W 变化；条件总效应为 c′加条件间接效应。

`probe_sd` 默认 1，必须为正；在 W 均值及均值 ± 指定倍数的样本标准差探查。输出使用 W 原始单位，`in_observed_range` 标记是否外推。`replications` 默认 1000，0 禁用效应推断，否则至少 2 次；`seed` 默认 42，非负整数。

**result** 包含两个方程的 OLS 系数、经典同方差协方差、拟合指标，以及 `details` 中的直接效应、三个条件间接/总效应和 `moderated_mediation_index`。方程零系数使用双侧 t 检验、n−k 自由度和 95% 区间。效应及指数的标准误与 95% 百分位区间来自整行配对 Bootstrap（Type-7 的 2.5%/97.5% 分位）；不输出效应 p 值。指数区间不含 0 是间接效应随 W 线性变化的区间证据，多个探查点未做多重比较校正。

每次重采样使用相同抽样行拟合两个方程，并固定原样本中心与探查点；拟合失败立即报错，不丢弃失败重复。**observations** 保留所有行及两个方程的响应、拟合值、残差，列为 `observation, response, fitted, residual, mediator, mediator_fitted, mediator_residual`。

输入须为对齐、无缺失的有限数值，设计满秩且 n>k。无固定行数上限。当前不包含两阶段同时调节、直接路径调节、多中介、潜变量或聚类推断；关联模型本身不证明因果机制。
