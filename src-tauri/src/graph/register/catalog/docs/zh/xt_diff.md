# XT Diff

在 **XT Align** 后的 **DataFrame** 上，按 **entity** 做一阶差分（面板内 Stata `D.` 语义）。

仅保留有效差分行；数值列逐实体在时间上差分。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **Aligned DataFrame** | 输入 | 经 **XT Align** 的平衡面板 |
| **Entity Col** | 输入 | 实体 ID 列名 |
| **Time Col** | 输入 | 时间列名 |
| **Diff** | 输出 | 差分后的 `DataFrame` |

## 用法

须先 **XT Align** 再差分。**Entity Col** 与 **Time Col** 须与对齐时一致。
