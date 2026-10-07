# Prais–Winsten Summary

接收 `yssbi.statistics.prais.fit` 产生的已拟合 `model`，基于该模型输出 `result`。不接收原始数据或估计配置，不重新拟合。

报告包含 AR(1) 误差方程 u[t]=ρu[t−1]+ε[t]、实际变换方式及每次迭代的完整精度 rho 历史。Rho 是序列相关参数，不为它编造标准误。可选系数约束使用拟合残差自由度下的 t/F 推断，包含 Cochrane–Orcutt 丢弃首条观测后的自由度。
