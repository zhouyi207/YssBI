# 森林图

连接至少两项独立研究的 effects 和正 variances。estimator 默认 paule_mandel，也可选 fixed 或 der_simonian_laird。confidence_level 默认 0.95，须严格介于 0 和 1；inference 默认 wald，可选 knapp_hartung 计算合并区间。
研究区间为 $y_i\pm z\sqrt{v_i}$，Pooled 行使用所选模型的系数区间。exponentiate 默认 false，仅对对数比值效应开启以显示比值；合并始终在输入尺度进行。
result 为系数区间图，包含 data（label、value、lower、upper）和 confidenceLevel。研究标签是从 1 开始的行位置，保留全部研究与合并行；图中不使用方块面积表示权重，也不使用菱形表示合并区间。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
