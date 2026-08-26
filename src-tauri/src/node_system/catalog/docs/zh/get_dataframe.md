# Get DataFrame

从项目数据库按节点实例参数加载 **DataFrame**，并在输出 pin 上发出表引用，供下游数据节点使用。

## 用法

将 **Get DataFrame** 放在数据流起点。在检查器中选择目标表，再将 **DataFrame** 接到 **Decompose DataFrame**、**Filter DataFrame**、回归节点等下游节点。所选表的 schema 会向下传播，以便动态 pin 正确解析。
