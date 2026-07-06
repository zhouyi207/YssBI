# Event Begin

**Event** 图的入口。图执行时从此处开始，经 **Out** 向外传递执行流——无 exec 输入 pin。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **Out** | Exec 输出 | 事件图执行时触发的首个 exec pin |

## 用法

每个事件驱动子图放置一个 **Event Begin**。将 **Out** 接到 **Sequence**、**Branch**、**Print** 等 exec 节点，定义事件触发时的执行顺序。纯数据流水线不需要此节点；它用于控制流与副作用图。
