# TS Align

将 **DataFrame** 对齐到规则时间网格，补齐缺失时间点并拒绝重复时间键。

时间列须为 **Int64** 或 **Date**；输出与输入同 schema 的 **Aligned** `DataFrame`。

## 用法

在 **TS Diff**、**TS Lag** 等严格时间操作前先对齐。缺失时间点填 null；重复时间键会报错。
