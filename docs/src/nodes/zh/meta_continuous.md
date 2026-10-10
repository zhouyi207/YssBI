# 连续型效应量

连接同一研究表中的 treatment_mean、treatment_sd、treatment_n 和 reference_mean、reference_sd、reference_n。每行表示两个独立组；各组样本量须为至少 2 的整数。标准差须非负，允许单组标准差为零，但效应量的最终方差须为正。
effect_measure 默认 mean_difference：$y=\bar x_T-\bar x_C$，$v=s_T^2/n_T+s_C^2/n_C$。hedges_g 使用 $g=J(\nu)(\bar x_T-\bar x_C)/s_p$，其中 $s_p$ 为合并标准差，$\nu=n_T+n_C-2$，精确校正因子 $J(\nu)=\Gamma(\nu/2)/[\sqrt{\nu/2}\Gamma((\nu-1)/2)]$；大样本方差为 $v=1/n_T+1/n_C+g^2/[2(n_T+n_C)]$。
各研究须采用相同结局及对照方向。此节点不估计配对组或变化值的效应。

confidence*level 默认 0.95，须严格介于 0 和 1。result 报告 measure、studies、confidence_level。studies 是可继续连接的表，包含 study（从 1 开始的输入行号）、effect、variance、standard_error、lower、upper；区间为所选尺度上的 $y\pm z*{(1+c)/2}\sqrt v$。这些是各研究估计值，不包含合并显著性检验。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
