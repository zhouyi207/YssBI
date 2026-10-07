# VEC Fit

连接至少两条按时间顺序排列、对齐、有限的 I(1) 序列。配置协整秩 rank、水平模型滞后阶数 lags，以及确定性项 trend（none、constant、trend）。

唯一输出 model 保留 Johansen VEC 的协整向量、调整系数、短期估计、推断统计、设计及残差。VEC Summary 投影这些拟合事实，仅在选中后计算残差 LM 或稳定性分析，不重新拟合。当前不提供单条 fitted/residuals 数列。
