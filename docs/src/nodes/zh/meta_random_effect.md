# 随机效应 Meta 分析

连接独立研究的 effects 和正 variances，各研究须估计可比较的效应。模型加入研究间方差，权重 $w_i=1/(v_i+\tau^2)$，合并效应 $\hat\mu=\sum w_i y_i/\sum w_i$。
estimator 默认 paule*mandel，求解 $\sum w_i(y_i-\hat\mu)^2=k-1$，无需正根时取零；der_simonian_laird 使用 $\max(0,(Q-k+1)/C)$，其中 $Q$ 来自固定权重，$C=\sum1/v_i-\sum(1/v_i)^2/\sum1/v_i$。
至少三项研究时，prediction_interval 使用 $\hat\mu\pm t*{k-2}\sqrt{\hat\tau^2+\operatorname{Var}(\hat\mu)}$，近似表示新研究真实效应的区间，与平均效应的置信区间含义不同。

confidence*level 默认 0.95，须严格介于 0 和 1。inference 默认 wald，系数检验为 $H_0:\beta_j=0$ 对 $\beta_j\ne0$，采用正态 Wald 推断；knapp_hartung 将协方差乘以 $Q(\hat\tau^2)/(k-p)$，检验与区间使用 $t*{k-p}$，单纯合并时 $p=1$。残差尺度小于 1 时其区间可能更窄。方差推断以已拟合 τ² 为条件。

result 包含 studies、estimator、inference、coefficients（估计值、标准误、统计量、P 值、区间）、covariance、residual_degrees_of_freedom、residual_q、heterogeneity、prediction_interval。heterogeneity 报告固定权重 Q，以及共同效应对异质性备择的卡方 P 值（$df=k-p$）、I²、H²、τ²。studies 表包含 study、effect、variance、standard_error、lower、upper、weight（和为 1）、fitted、residual。至少需要两项研究，且设计矩阵满秩。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
