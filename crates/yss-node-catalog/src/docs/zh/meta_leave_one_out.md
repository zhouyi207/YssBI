# 逐研究剔除分析

连接至少三项独立研究的 effects 和正 variances。每次剔除一项研究后重新拟合合并模型，包括重新估计 τ²。estimator 默认 paule_mandel，也可选 fixed 或 der_simonian_laird。
result 是完整数据的基线模型。将其合并效应及异质性与 studies 表比较，识别影响较大的研究；剔除分析用于敏感性评估，不是自动删除研究的规则。

confidence_level 默认 0.95，须严格介于 0 和 1。inference 默认 wald，系数检验为 $H_0:\beta_j=0$ 对 $\beta_j\ne0$，采用正态 Wald 推断；knapp_hartung 将协方差乘以 $Q(\hat\tau^2)/(k-p)$，检验与区间使用 $t_{k-p}$，单纯合并时 $p=1$。残差尺度小于 1 时其区间可能更窄。方差推断以已拟合 τ² 为条件。

studies 包含各次重拟合的 omitted_study（从 1 开始的输入行号）、estimate、standard_error、lower、upper、tau_squared、i_squared_percent。基线报告零效应系数检验及 Cochran Q 同质性检验；剔除行提供区间与异质性数值，不另设影响程度的 P 值。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
