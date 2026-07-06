# TS Align

将 **DataFrame** 对齐到规则时间网格，补齐缺失时间点并拒绝重复时间键。

时间列须为 **Int64** 或 **Date**；输出与输入同 schema 的 **Aligned** `DataFrame`。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataFrame** | 输入 | 含时间列的源表 |
| **Time Series Name** | 输入 | 时间列名（`String`） |
| **Interval** | 输入 | 步长；省略时从数据推断 |
| **Aligned** | 输出 | 对齐后的 `DataFrame` |

## 用法

在 **TS Diff**、**TS Lag** 等严格时间操作前先对齐。缺失时间点填 null；重复时间键会报错。
