# Print

在图执行过程中将 **Message** 字符串写入应用日志。记录完成后执行流从 **In** 继续到 **Out**。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **In** | Exec 输入 | 触发写日志 |
| **Message** | 输入（可选） | 写入日志的文本 |
| **Out** | Exec 输出 | 日志写入后触发 |

## 用法

在 exec 链上插入 **Print** 用于调试或进度标记。**Message** 可接 **String** 常量或字符串 pin，也可依赖节点默认值。将 **Out** 链接到下一 exec 节点（**Sequence** 某步、**Branch**、**View** 等）。
