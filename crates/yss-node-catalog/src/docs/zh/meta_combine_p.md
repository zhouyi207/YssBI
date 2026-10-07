# P 值合并

连接位于 $[0,1]$ 的 p_values；研究须独立且检验具有可比较的科学零假设。p_method 默认 fisher，统计量 $X=-2\sum\log p_i$ 在“各研究零假设均成立”的联合零假设下服从 $\chi^2_{2k}$，备择为至少一项研究偏离零假设。
stouffer 合并方向一致的单侧 P 值：$Z=\sum w_i\Phi^{-1}(1-p_i)/\sqrt{\sum w_i^2}$，零假设下服从 $N(0,1)$，备择为正向偏离。可选 weights 输入提供正有限权重，不连接则等权；Fisher 忽略权重。
result 包含 method、studies、statistic、degrees_of_freedom、p_value。极限统计量为无限时以 null 表示，并保留极限 P 值；Stouffer 同时遇到 0 和 1 无定义。输出是合并显著性，不是效应量。
