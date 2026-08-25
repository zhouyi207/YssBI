# XT Align

按完整 $(entity \times time)$ 网格对齐面板 **DataFrame**；缺失格填 null。

实体列：**Categorical**、**Int64** 或 **String**；时间列：**Int64** 或 **Date**。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataFrame** | 输入 | 面板源表 |
| **Entity Col** | 输入 | 实体 ID 列名 |
| **Time Col** | 输入 | 时间列名 |
| **Interval** | 输入 | 可选；时间步长 |
| **Aligned** | 输出 | 平衡面板 `DataFrame` |

## 用法

面板回归或 **XT Diff** 前的标准步骤。输出 schema 与输入一致，行数扩展为各实体 × 各时间点的笛卡尔积。
