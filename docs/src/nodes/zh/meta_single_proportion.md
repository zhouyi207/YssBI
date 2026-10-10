# 单个率效应量

连接 events 与 sample_size，要求整数计数 $0\le x\le n$、$n\ge1$。effect_measure 默认 logit_proportion：$y=\log[p/(1-p)]$，$v=1/x+1/(n-x)$；proportion 使用 $y=p=x/n$，$v=p(1-p)/n$；arcsine_proportion 使用 $y=\arcsin\sqrt p$，$v=1/(4n)$。
continuity_correction 默认 0.5，须非负。原始比例与 logit 比例遇到边界研究（$x=0$ 或 $n$）时使用 $x+c,n+2c$；反正弦口径使用原始计数。校正设为 0 可能产生无法合并的零/无限方差。区间是在所选尺度上的正态区间，不裁剪边界；logit 或反正弦结果需反变换后才可解释为比例。

confidence*level 默认 0.95，须严格介于 0 和 1。result 报告 measure、studies、confidence_level。studies 是可继续连接的表，包含 study（从 1 开始的输入行号）、effect、variance、standard_error、lower、upper；区间为所选尺度上的 $y\pm z*{(1+c)/2}\sqrt v$。这些是各研究估计值，不包含合并显著性检验。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
