# τ² 研究间方差

连接至少两项独立研究的 effects 和正 variances。estimator 默认 paule*mandel，使用 $w_i=1/(v_i+\tau^2)$ 求解 $Q(\tau^2)=k-1$；der_simonian_laird 使用 $\hat\tau^2=\max(0,(Q(0)-k+1)/C)$，$C=\sum1/v_i-\sum(1/v_i)^2/\sum1/v_i$。当 $Q(0)\le k-1$ 时均取零。
result 的 tau_squared 位于效应量的平方尺度，同时给出 q、degrees_of_freedom、p_value、i_squared_percent、h_squared。P 值通过 $Q(0)\sim\chi^2*{k-1}$ 检验共同效应，备择为异质性，不是针对 τ² 边界的校正检验。节点提供 τ² 点估计，不提供其置信区间。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
