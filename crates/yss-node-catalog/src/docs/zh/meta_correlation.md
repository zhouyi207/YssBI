# 相关系数效应量

连接同一研究表中的 correlation 与 sample_size。Pearson 相关系数须满足 $|r|<1$，样本量为大于 3 的整数。效应使用 Fisher 变换 $y=\operatorname{atanh}(r)$，方差为 $v=1/(n-3)$。应合并变换后的效应，再对合并值及区间端点取 $\tanh$ 还原相关系数。输入表示独立的未调整相关，不适用于未提供协变量信息的偏相关。

confidence_level 默认 0.95，须严格介于 0 和 1。result 报告 measure、studies、confidence_level。studies 是可继续连接的表，包含 study（从 1 开始的输入行号）、effect、variance、standard_error、lower、upper；区间为所选尺度上的 $y\pm z_{(1+c)/2}\sqrt v$。这些是各研究估计值，不包含合并显著性检验。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
