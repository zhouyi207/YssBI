# 共线性分析

连接一个或多个对齐的数值自变量到 **variables**。至少需要两个观测；允许变量数超过观测数，以报告秩不足。缺失或非有限值会报错。

**包含截距**默认开启：中心化自变量并加入截距；关闭时检查未中心化设计。计算前各设计列缩放为单位长度，不改变列空间。该节点不需要因变量或预先拟合的模型。

对缩放后设计 $Z$，报告其秩、条件数及 $Z^\mathsf{T}Z$ 的降序特征值 $\lambda_j$。条件指数为 $\sqrt{\lambda_{\max}/\lambda_j}$。默认中心化 VIF 为 $1/(1-R_j^2)$，其中 $R_j^2$ 来自该变量对其余变量的辅助回归；关闭截距时使用未中心化口径。

**result** 包含 observations、columns、rank、full_column_rank、centered、condition_number、eigenvalues、condition_indices 和逐列 terms（term、constant、vif、tolerance）。

截距及零列的 VIF/容忍度为 null；完全依赖的变量 VIF 为 null、容忍度为 0。无界条件数或条件指数为 null，不使用有限大数替代无穷。近零特征值按数值分解精度处理。它是设计诊断，不输出 p 值；高 VIF 不直接决定删变量。
