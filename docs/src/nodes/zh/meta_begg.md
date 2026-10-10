# Begg 检验

连接至少三项独立研究的 effects 和正 variances。设 $\hat\mu$ 为固定权重合并效应，$V=1/\sum1/v_i$；对调整效应 $z_i=(y_i-\hat\mu)/\sqrt{v_i-V}$ 与 $v_i$ 计算 Kendall tau-b。
零假设为调整效应与方差没有秩关联，备择为存在非零关联。使用考虑结值的渐近正态检验，不进行精确置换或连续性校正；两个秩变量均需有变异。
result 包含 coefficient、observations、inference（统计量、p_value、方法）、concordant_pairs、discordant_pairs 和结值对数。它诊断小样本研究不对称性，拒绝零假设不代表已经证实发表偏倚。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
