# 多元方差分析（MANOVA）

联合比较多个响应变量的组效应。输入 2–16 个数值 **Response** 和 1–8 个 **Factor** 数列。所有列逐行对应，关系输入共享行域，拒绝缺失值。每个因素有 2–32 个观测类别。设计含截距，须满秩、最多 256 列且有正残差自由度；残差 SSCP 必须正定，完全共线的响应变量不能用于联合检验。

**因素效应** 默认 `full_factorial`，可切换为 `main_effects`。**平方和类型** 默认 `type_iii`；I 型按主效应输入顺序及交互阶数进行顺序检验；II 型调整除本项高阶关联项外的其他项；III 型使用和为零对比调整全部其他项。

每项的原假设是其对比系数在所有响应变量上均为零，备择是假设矩阵至少一个系数非零。令 $E$ 为最终模型的残差平方和与交叉乘积矩阵，$H=E_R-E_A$ 为该项的假设 SSCP，$\lambda_i$ 为 $E^{-1}H$ 的特征根：

$$\Lambda=\prod_i(1+\lambda_i)^{-1},\qquad V=\sum_i\frac{\lambda_i}{1+\lambda_i},\qquad U=\sum_i\lambda_i,\qquad \Theta=\max_i\lambda_i.$$

依次为 Wilks Lambda、Pillai trace、Hotelling–Lawley trace 和 Roy greatest root。推断采用标准 F 变换/近似，要求独立观测、近似多元正态误差及组间相同的协方差矩阵。Roy 的 F 近似是上界，对多维假设可能偏乐观。

唯一 **Result** 含 `table` 的逐项 `hypothesis_df`、`hypothesis_sscp` 与四种 `tests`，以及 `error_df`、`error_sscp`、响应数和原始因素标签。SSCP 按输入响应顺序排列为行优先的嵌套数组。每种检验包含 `statistic`、F、分子/分母自由度和 p 值；若小样本下 F 近似无有效自由度，推断字段为 null，统计量仍保留。小 p 值支持响应均值向量的差异，不能识别具体响应或类别对。

检验定义及 F 变换参见 [statsmodels 多元检验](https://www.statsmodels.org/stable/_modules/statsmodels/multivariate/multivariate_ols.html)。
