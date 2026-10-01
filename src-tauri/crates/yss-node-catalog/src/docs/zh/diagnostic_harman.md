# 共同方法偏差（Harman 单因子）

将同一批受访者的全部待考察题项连接到 **variables**。至少两个数值题项、两个观测；所有列必须对齐且有变异，缺失或非有限值会报错。

本节点明确采用**相关矩阵上的未旋转主成分法**：先中心化并按样本标准差标准化，再分解相关矩阵。无配置参数，不执行验证性因子分析。

若相关矩阵特征值为 $\lambda_1\geq\cdots\geq\lambda_p$，第一主成分解释率为 $\lambda_1/\sum_j\lambda_j$，载荷为 $\sqrt{\lambda_1}v_1$，其中 $v_1$ 为对应单位特征向量。

**result** 输出 extraction、observations、variables、eigenvalues、explained_variance_ratio、first_component_ratio、first_component_loadings 和 eigenvalues_above_one。载荷按输入题项顺序排列，解释率使用 0–1 比例。

较高的第一主成分解释率表示共同变异集中，但该诊断不能单独证实或排除共同方法偏差。结果不输出 p 值，也不以固定阈值自动判定“通过”。
