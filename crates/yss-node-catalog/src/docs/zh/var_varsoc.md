# VAR 滞后阶选择

连接至少两条按时间顺序排列、对齐且有限的数值序列，以 max_lags 设置最大候选滞后阶数。在估计器的共同样本上，对 0 到 max_lags 报告 LL、LR、FPE、AIC、HQIC、SBIC，据此设置 VAR Fit 的 lags。
