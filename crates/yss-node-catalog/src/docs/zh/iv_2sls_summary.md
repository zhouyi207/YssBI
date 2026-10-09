# IV 2SLS Summary

连接 IV 2SLS Fit 产生的 `model`。Configure 默认选择模型概览和系数推断；可开启 `first_stage`、`overidentification` 或 `endogeneity`，仅计算所选分析，复用同一拟合样本和模型设定。

`result` 只包含所选内容。不接收原始数据或估计配置，不重新拟合 IV 模型。过度识别检验需要额外工具变量，当前内生性检验要求 nonrobust 协方差；不可用统计量显示为 null。

系数检验以 $H_0:\beta_j=0$ 对 $H_1:\beta_j\ne0$，统计量为 $\hat\beta_j/\operatorname{SE}(\hat\beta_j)$，采用所选协方差。默认参考标准正态分布，`small=true` 时参考自由度为 $n-k$ 的 Student-t；95% 区间临界值使用同一参考分布。其中 $n$ 为观测数，$k$ 为全部估计系数数目，包含截距。

整体检验以 $H_0:\beta_s=0$ 对至少一个非零斜率，统计量为 $W=\hat\beta_s^{\mathsf T}V_s^{-1}\hat\beta_s$；$V_s$ 为斜率协方差，$q$ 为被检系数数目，不含估计的截距。默认 $W$ 参考 $\chi^2(q)$；`small=true` 时 $F=W/q$ 参考 $F(q,n-k)$。`model.statistics.modelTest` 记录 `distribution`（`chiSquared` 或 `f`）、`statistic`、`pValue` 以及 `df` 或 `dfNumerator`/`dfDenominator`。较小的 p 值表示拒绝所述原假设。

开启 `overidentification` 后，原假设为工具变量与结构误差正交，备择为至少一个约束不成立。nonrobust 协方差报告 Sargan $S=n(1-e^{\mathsf T}e/u^{\mathsf T}u)$ 和 Basmann $B=S(n-k_Z)/(n-S)$，均参考 $\chi^2(m)$；其中 $u$ 为模型保存的结构残差，$e$ 为其对全部工具变量进行辅助回归后的残差，$k_Z$ 为该回归的自变量数（含截距），$m$ 为排除工具变量数减内生变量数。这两个检验假定误差独立且同方差。稳健协方差下，当前 Wooldridge 得分检验假定观测独立：$W=n-\mathrm{RSS}_1$ 参考 $\chi^2(m)$；$\mathrm{RSS}_1$ 来自常数一向量对 $\hat q_j u$ 的回归，$m$ 个独立的 $\hat q_j$ 方向覆盖全部排除工具变量对已包含自变量及拟合内生变量回归后的残差空间。HC0–HC3 使用同一渐近得分检验，其系数调整保留在系数表中。工具变量列顺序不改变检验结果；未定义的约束方向或得分协方差使所选分析失败。较小的 p 值拒绝工具变量有效性或结构模型设定。响应量纲变化保持这些检验结果。结构残差全零时整项分析不可用；Basmann 还要求 $n>k_Z$ 及正的辅助残差变异，否则其字段为 null，Sargan 仍可报告。

开启 `first_stage` 后，每个内生变量对已包含自变量和工具变量进行回归，采用所选协方差。系数检验参考自由度为 $n-k_Z$ 的 Student-t，不受结构模型 `small` 控制。单个内生变量时，$H_0:\pi_2=0$ 检验全部排除工具变量系数为零，对至少一个非零系数；$F=\hat\pi_2^{\mathsf T}V_2^{-1}\hat\pi_2/q$ 参考 $F(q,n-k_Z)$。其中 $k_Z$ 为全部第一阶段系数数目，$q$ 为排除工具变量数，$V_2$ 为其所选协方差。`firstStage.equations` 保留 `inference` 和 `df_residual`。无截距时 R² 不中心化。未定义的第一阶段推断使所选分析失败；关闭该选项仍可汇总结构模型。

开启 `endogeneity` 后，原假设为全部被检内生变量可视为外生，备择为至少一个不能视为外生。令 $R_0$ 为 OLS 残差平方和，$R_1$ 为加入第一阶段残差后的残差平方和，$L=R_0-R_1$；$q$ 为被检内生变量数：

$$
D=\frac{nL}{R_0}\sim\chi^2(q),\qquad
F_{\mathrm{WH}}=\frac{L/q}{R_1/(n-k-q)}\sim F(q,n-k-q).
$$

`endogeneity.endogenous` 保留 `durbin_stat`、`durbin_p_value`、`wu_stat`、`wu_p_value`、`df` 和 `wu_df_denom`。较小的 p 值拒绝外生性。剩余自由度非正或增广残差变异无法分辨时，Wu 字段为 null，Durbin 仍可报告；缺少可估计的检验方向时，这个联合记录为 null。`endogeneity.hausman` 独立保留采用共同 OLS 方差、实际协方差差秩的比较检验。响应量纲变化保持检验结果；本项内生性分析仍不支持稳健协方差。

报告保留实际系数/工具变量名，展示结构方程、第一阶段方程及命名系数、弱工具变量临界值。不可用的识别检验或依赖协方差的方法显示明确原因。可选 `hypothesis_test`/`hypothesis` 使用模型协方差检验任意独立系数约束：默认 z/χ²，`small=true` 时为 t/F。
