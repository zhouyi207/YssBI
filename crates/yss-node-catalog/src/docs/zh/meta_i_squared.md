# I² 异质性统计量

连接至少两项独立研究的 effects 和正 variances。由固定权重 Cochran $Q$ 与 $df=k-1$ 计算 $I^2=100\max(0,(Q-df)/Q)$（$Q=0$ 时取零），以及 $H^2=\max(1,Q/df)$。
result 包含 i_squared_percent、h_squared、q、degrees_of_freedom、p_value。该 P 值来自 Cochran 同质性检验：零假设为真实效应相同，$Q\sim\chi^2_{k-1}$，备择为存在异质性，并非另做 I² 检验。此节点不拟合随机效应方差，因此 tau_squared 为零。I² 表示相对异质性，不是效应的绝对方差。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
