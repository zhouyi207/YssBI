# Probit Summary

接收 `yssbi.statistics.probit.fit` 产生的已拟合 `model`，基于该模型输出 `result`。不接收原始数据或估计配置，不重新拟合。

启用 `marginal_effects` 后计算边际效应（默认关闭，避免普通汇总额外承担高维计算）；求值方式默认 `average`（AME）；`at_means` 使用解释变量均值（MEM）。`marginal_at` 支持 `x1 = 0.75` 等赋值；未指定变量保留原观测或均值。`dydx` 为 ∂p/∂x，`eydx` 为 ∂log(p)/∂x，`dyex` 为 x·∂p/∂x，`eyex` 为 x·∂log(p)/∂x。这些是连续变量导数；数值 0/1 列也按连续变量计算，不代表离散类别对比。标准误对最终平均效应使用解析 Delta 方法；z 检验和 95% 区间使用正态分布，不包含截距。不为未定义比值或零方差推断编造数值。

分类阈值默认 0.5、范围 [0,1]，p ≥ 阈值时预测为 1。估计样本分类表提供 TP/FP/FN/TN、敏感度、特异度、PPV、NPV、准确率和错误率；分母为零时该比率不可用。这些是样本内表现，不是样本外验证。

`hypothesis_test` 启用命名系数约束（`hypothesis` 默认 `x1 = 0`）。单等式使用双侧 z 检验，不等式使用相应单侧正态尾概率，多等式使用 Wald χ²，自由度为独立约束数。冗余约束报错；变量名使用模型显示的系数标签。

[参考：statsmodels 离散模型边际效应](https://www.statsmodels.org/v0.14.6/generated/statsmodels.discrete.discrete_model.DiscreteResults.get_margeff.html)。
