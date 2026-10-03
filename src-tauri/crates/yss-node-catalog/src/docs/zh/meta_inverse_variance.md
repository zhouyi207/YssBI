# 一般倒方差 Meta 分析

连接同一研究表中的 effects 与严格为正的抽样 variances。estimator 默认 fixed，也可选 der_simonian_laird 或 paule_mandel。合并效应为 $\hat\mu=\sum w_i y_i/\sum w_i$，$w_i=1/(v_i+\tau^2)$；固定效应令 $\tau^2=0$。
DerSimonian–Laird 使用 $\hat\tau^2=\max(0,(Q-k+1)/C)$，其中 $C=\sum1/v_i-\sum(1/v_i)^2/\sum1/v_i$。Paule–Mandel 求解 $Q(\tau^2)=k-1$，若 $Q(0)\le k-1$ 则取零。

confidence_level 默认 0.95，须严格介于 0 和 1。inference 默认 wald，系数检验为 $H_0:\beta_j=0$ 对 $\beta_j\ne0$，采用正态 Wald 推断；knapp_hartung 将协方差乘以 $Q(\hat\tau^2)/(k-p)$，检验与区间使用 $t_{k-p}$，单纯合并时 $p=1$。残差尺度小于 1 时其区间可能更窄。方差推断以已拟合 τ² 为条件。

result 包含 studies、estimator、inference、coefficients（估计值、标准误、统计量、P 值、区间）、covariance、residual_degrees_of_freedom、residual_q、heterogeneity、prediction_interval。heterogeneity 报告固定权重 Q，以及共同效应对异质性备择的卡方 P 值（$df=k-p$）、I²、H²、τ²。studies 表包含 study、effect、variance、standard_error、lower、upper、weight（和为 1）、fitted、residual。至少需要两项研究，且设计矩阵满秩。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
