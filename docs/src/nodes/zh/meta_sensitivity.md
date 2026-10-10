# Meta 敏感性分析

连接至少三项独立研究的 effects 和正 variances。result 包含 baseline（所选估计方法，默认 paule_mandel）和 alternative_models（使用完整数据拟合的固定效应、DerSimonian–Laird、Paule–Mandel 三种模型），均使用所选 inference 与 confidence_level。
studies 表每次剔除一项研究后重拟合所选基线模型，并重新估计异质性。比较不同假设下的估计变化、区间及 τ²；节点不会自动筛选研究或选定最佳模型。

confidence*level 默认 0.95，须严格介于 0 和 1。inference 默认 wald，系数检验为 $H_0:\beta_j=0$ 对 $\beta_j\ne0$，采用正态 Wald 推断；knapp_hartung 将协方差乘以 $Q(\hat\tau^2)/(k-p)$，检验与区间使用 $t*{k-p}$，单纯合并时 $p=1$。残差尺度小于 1 时其区间可能更窄。方差推断以已拟合 τ² 为条件。

studies 包含各次重拟合的 omitted_study（从 1 开始的输入行号）、estimate、standard_error、lower、upper、tau_squared、i_squared_percent。基线报告零效应系数检验及 Cochran Q 同质性检验；剔除行提供区间与异质性数值，不另设影响程度的 P 值。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
