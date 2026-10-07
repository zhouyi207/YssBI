# AHP 层次分析

将同一个方形判断矩阵的列依次连接到 **criteria**，行列须使用相同指标顺序。
A(i,j) 表示指标 i 相对 j 的重要性：数值须为正、对角线为 1、对称位置互为倒数，
采用 1e−8 的对数相对容差。缺失、非有限数值和观测数不一致会报错。

通过正的主特征向量归一化得到权重。
**result** 输出最大特征根、CI = (λ − n)/(n − 1)、RI、CR = CI/RI
及 CR ≤ 0.1 是否成立；单指标时 CI 为 0。
**weights** 按指标顺序输出，可连接后续评分节点。

**random_index** 为 0 时，3–15 阶使用 Tummala–Ling 随机一致性表；
设为正数可指定 RI。没有可用 RI 时仅 CR 及其判定为空，权重和 CI 仍正常计算，
不限制矩阵阶数。一个节点分析一个层级；多专家判断应先取几何平均，
多个层级的局部权重需另行组合。

参考：[PyMCDM AHP 与 RI 定义](https://pymcdm.readthedocs.io/en/latest/pymcdm.weights.html)。
