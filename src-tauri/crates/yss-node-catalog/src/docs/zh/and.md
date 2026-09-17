# And（&&）

两布尔值的逻辑与：

$$
\text{Result} = A \land B
$$

| $A$   | $B$   | 结果  |
| ----- | ----- | ----- |
| false | false | false |
| false | true  | false |
| true  | false | false |
| true  | true  | true  |

支持二元语义的标量、内存数据序列和惰性数据序列。输入为序列时逐元素计算；AND/OR 支持任一侧标量广播，内存序列必须等长，惰性序列必须属于同一关系行域。惰性结果在读取或下游消费时计算，不修改源数据集。采用三值逻辑：`false AND null = false`、`true AND null = null`、`true OR null = true`、`false OR null = null`、`NOT null = null`。这些节点合并数据条件，不保证跳过上游节点的求值。

## 用法

在筛选或其他下游数据转换前合并多个布尔掩码。将 **Equal** 等比较结果接入 **A**、**B**。
