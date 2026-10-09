# IV 2SLS Summary

连接 IV 2SLS Fit 产生的 `model`。Configure 默认选择模型概览和系数推断；可开启 `first_stage`、`overidentification` 或 `endogeneity`，仅计算所选分析，复用同一拟合样本和模型设定。

`result` 只包含所选内容。不接收原始数据或估计配置，不重新拟合 IV 模型。过度识别检验需要额外工具变量，当前内生性检验要求 nonrobust 协方差；不可用统计量显示为 null。

系数检验以 $H_0:\beta_j=0$ 对 $H_1:\beta_j\ne0$，统计量为 $\hat\beta_j/\operatorname{SE}(\hat\beta_j)$，采用所选协方差。默认参考标准正态分布，`small=true` 时参考自由度为 $n-k$ 的 Student-t；95% 区间临界值使用同一参考分布。其中 $n$ 为观测数，$k$ 为全部估计系数数目，包含截距。

整体检验以 $H_0:\beta_s=0$ 对至少一个非零斜率，统计量为 $W=\hat\beta_s^{\mathsf T}V_s^{-1}\hat\beta_s$；$V_s$ 为斜率协方差，$q$ 为被检系数数目，不含估计的截距。默认 $W$ 参考 $\chi^2(q)$；`small=true` 时 $F=W/q$ 参考 $F(q,n-k)$。`model.statistics.modelTest` 记录 `distribution`（`chiSquared` 或 `f`）、`statistic`、`pValue` 以及 `df` 或 `dfNumerator`/`dfDenominator`。较小的 p 值表示拒绝所述原假设。

报告保留实际系数/工具变量名，展示结构方程、第一阶段方程及命名系数、弱工具变量临界值。不可用的识别检验或依赖协方差的方法显示明确原因。可选 `hypothesis_test`/`hypothesis` 使用模型协方差检验任意独立系数约束：默认 z/χ²，`small=true` 时为 t/F。
