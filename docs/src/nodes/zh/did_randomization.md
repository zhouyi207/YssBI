# DID 伪处理组随机化检验

连接对齐、有限的 Y、可选 X₁, X₂, …、entity、time、treat、post 数列。treat/post 必须为 0 或 1，treat 在同一实体内保持不变，实体与时间组合不可重复。

节点先拟合实际 Treat×Post 的双向固定效应模型并按实体聚类推断，再在保持处理实体数量的条件下随机分配处理组。repetitions 为 10–2000，默认 100；seed 为非负整数，默认 42。相同输入与种子得到相同结果。

result 包含实际系数、置换系数的均值和标准差、有效置换次数、随机化 p 值或结构化不可用原因；至少需要 10 次有效置换。每次置换之间检查取消。本节点执行伪处理组安慰剂检验，不执行事件研究。

现可配置 `constant`（true）、`covariance`（cluster）以及可选 `use_observed_coefficient`/`observed_coefficient`（false/0）。指定观测系数会改变随机化参考阈值，不会重新拟合该指定效应；未指定时拟合观测 TWFE 系数。报告包含 treat/post 名称与所选配置。ATT 标签指 TWFE 处理×事后系数；因果 ATT 解释仍需满足相应 DID 识别假设。
