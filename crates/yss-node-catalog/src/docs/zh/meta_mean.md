# 平均值效应量

连接各独立研究的 mean、sd、sample_size。研究效应就是均值 $y=\bar x$，抽样方差 $v=s^2/n$。标准差须为正，样本量须为至少 2 的整数。输出表示单组均值，各研究须测量同一指标且单位一致。

confidence_level 默认 0.95，须严格介于 0 和 1。result 报告 measure、studies、confidence_level。studies 是可继续连接的表，包含 study（从 1 开始的输入行号）、effect、variance、standard_error、lower、upper；区间为所选尺度上的 $y\pm z_{(1+c)/2}\sqrt v$。这些是各研究估计值，不包含合并显著性检验。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
