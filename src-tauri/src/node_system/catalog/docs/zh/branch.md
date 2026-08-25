# Branch

条件控制流节点。执行到达 **In** 后，根据 **Condition** 仅触发 **True** 或 **False** 之一 exec 输出。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **In** | Exec 输入 | 上游执行流 |
| **Condition** | 输入（可选） | 布尔值；未连接时视为 false |
| **True** | Exec 输出 | **Condition** 为 true 时触发 |
| **False** | Exec 输出 | **Condition** 为 false 时触发 |

## 用法

将 **In** 接到上游 exec pin（如 **Event Begin**、**Sequence** 或 **Print**）。**Condition** 接布尔值或表达式结果。在 **True** / **False** 上分别连接对应分支。仅选中路径会执行，另一路保持空闲。
