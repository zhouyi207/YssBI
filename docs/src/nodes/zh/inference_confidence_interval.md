# 置信区间 CI

连接逐行对齐的数值 estimates 与 standard_errors。输入须非空且有限，标准误须非负。每行表示已计算好的估计值和标准误；节点不会从原始样本重新估计抽样方差。

confidence_level 默认 0.95，须严格介于 0 和 1。degrees_of_freedom 默认 0，表示使用正态参考分布；正有限值表示使用相应自由度的 Student t 分布，适用于所有输入行。

置信水平 $c$ 对应区间为 $\hat\theta\pm q_{(1+c)/2}s$，其中 $s$ 是输入标准误，$q$ 是标准正态或 t 分位数。零标准误给出点区间。解释依赖所提供标准误及参考近似的有效性。区间保留原始估计尺度，不裁剪到概率范围，也不进行多重性校正或尺度变换。

result 包含 rows、confidence_level、reference_distribution、degrees_of_freedom（正态口径为 null）。intervals 表包含 index（从 1 开始的输入行号）、estimate、standard_error、lower、upper，可分页及连接下游表节点。节点不凭这些输入额外生成 P 值或新的假设检验。
