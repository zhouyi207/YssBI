# XT Align

按完整 $(entity \times time)$ 网格对齐面板 **DataFrame**；缺失格填 null。

实体列：**Categorical**、**Int64** 或 **String**；时间列：**Int64** 或 **Date**。

## 用法

面板回归或 **XT Diff** 前的标准步骤。输出 schema 与输入一致，行数扩展为各实体 × 各时间点的笛卡尔积。
