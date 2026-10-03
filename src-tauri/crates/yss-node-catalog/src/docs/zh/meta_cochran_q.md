# Cochran Q 异质性检验

连接至少两项独立研究的 effects 和正 variances。设 $w_i=1/v_i$，$\hat\mu$ 为固定效应合并值，$Q=\sum w_i(y_i-\hat\mu)^2$ 在“真实效应相同”的零假设下近似服从 $\chi^2_{k-1}$。备择为研究间存在异质性，小 P 值拒绝同质性。
result 包含 q、degrees_of_freedom、p_value、i_squared_percent、h_squared、tau_squared（此固定权重诊断中为零）。研究少时 Q 检验功效较低，研究多时可能非常敏感，应结合 I² 和研究差异解释。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
