# Filter DataFrame

按布尔 **Condition** **DataSeries** 筛选 **DataFrame** 行，仅保留条件为 true 的观测。输出 schema 与输入表一致。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataFrame** | 输入 | 待筛选的表 |
| **Condition** | 输入 | 布尔 **DataSeries** 掩码；长度须等于行数 |
| **DataFrame** | 输出 | 筛选后的表，列与输入相同 |

## 用法

用比较节点（如 **DataSeries** 比较）或逻辑运算构造 **Condition**，使每行对应一个布尔值。连接 **DataFrame** 与 **Condition** 后执行图即可。掩码为 false 的行被丢弃；长度不一致或类型错误会报错。
