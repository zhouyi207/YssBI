# Branch

条件控制流节点。执行到达 **In** 后，根据 **Condition** 仅触发 **True** 或 **False** 之一 exec 输出。

## 用法

将 **In** 接到上游 exec pin（如 **Event Begin**、**Sequence** 或 **Print**）。**Condition** 接布尔值或表达式结果。在 **True** / **False** 上分别连接对应分支。仅选中路径会执行，另一路保持空闲。
