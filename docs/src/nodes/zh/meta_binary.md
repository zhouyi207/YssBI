# 二分类效应量

连接 treatment_events、treatment_n、reference_events、reference_n。计数须为非负整数，事件数不得超过对应的正样本量。以 $a,b,c,d$ 表示处理组事件/非事件和对照组事件/非事件，$n_T=a+b$，$n_C=c+d$。
effect_measure 默认 log_odds_ratio：$y=\log(ad/bc)$，$v=1/a+1/b+1/c+1/d$；log_risk_ratio 使用 $y=\log[(a/n_T)/(c/n_C)]$，$v=1/a-1/n_T+1/c-1/n_C$；risk_difference 使用 $y=a/n_T-c/n_C$，$v=ab/n_T^3+cd/n_C^3$。
continuity_correction 默认 0.5，须非负。某研究任一格为零时，先给该研究的四格同时加校正量；设为 0 则关闭校正。非有限效应或零方差不能进入合并分析。比值效应保留在对数尺度。

confidence*level 默认 0.95，须严格介于 0 和 1。result 报告 measure、studies、confidence_level。studies 是可继续连接的表，包含 study（从 1 开始的输入行号）、effect、variance、standard_error、lower、upper；区间为所选尺度上的 $y\pm z*{(1+c)/2}\sqrt v$。这些是各研究估计值，不包含合并显著性检验。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
