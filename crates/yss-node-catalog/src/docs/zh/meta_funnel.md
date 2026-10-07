# 漏斗图

连接至少两项独立研究的 effects 和正 variances。estimator 默认 paule_mandel，也可选 fixed 或 der_simonian_laird。confidence_level 默认 0.95，须严格介于 0 和 1。
横轴为效应 $y_i$，纵轴为标准误 $s_i=\sqrt{v_i}$，标准误为零的位置在顶部。参考边界为合并值周围的 $\hat\mu\pm z_{(1+c)/2}s$，并绘制中心线。这些是抽样误差轮廓，不是 τ² 的置信区间或正式发表偏倚检验。
result 包含 data、xLabel、yLabel、referenceLines、metadata。合并使用全部研究，最多显示 2048 个研究点，抽样情况在 metadata 中明确标注。不对称性需结合研究背景解释。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
