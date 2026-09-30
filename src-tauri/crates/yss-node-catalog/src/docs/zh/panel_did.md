# Panel DID (TWFE)

对因变量、可选自变量和预先计算的二元交互项 Treat×Post 做双向固定效应回归，吸收实体与时间效应。输入 response、predictors、entity、time、treatment 为对齐、等长、有限数值列；实体与时间组合不可重复。

treatment 必须是交互项，不能只连接处理组标记。推断按实体聚类。唯一的 result 输出包含 model 和 summary，保留拟合模型及其结构化汇总。

伪处理组检验请使用独立的 DID 随机化节点，分别输入 treat/post。本节点不提供事件研究估计。
