# Switch

按整数 **Selector** 分发执行。匹配的案例序号触发对应 **Case**；否则走 **Default**。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **In** | Exec 输入 | 上游执行流 |
| **Selector** | 输入（可选） | `Int64` 案例序号；默认 0 |
| **Case** *n* | Exec 输出（可增删） | `Selector == n` 时触发；默认 2 个 |
| **Default** | Exec 输出 | 序号为负或超出案例数时触发 |

## 用法

用 **Switch** 替代冗长的 **Branch** 链，处理离散整数模式（图表类型、模型族等）。
