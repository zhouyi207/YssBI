# Not（!）

逻辑非：

$$
\text{Result} = \lnot A
$$

| $A$   | 结果  |
| ----- | ----- |
| false | true  |
| true  | false |

支持二元语义的标量、内存数据序列和惰性数据序列。输入为序列时逐元素计算；AND/OR 支持任一侧标量广播；数列必须等长，按当前位置逐项计算，可混合数据库与内存数列。惰性结果在读取或下游消费时计算，不修改源数据集。采用三值逻辑：`false AND null = false`、`true AND null = null`、`true OR null = true`、`false OR null = null`、`NOT null = null`。这些节点合并数据条件，不保证跳过上游节点的求值。

## 用法

在筛选或其他下游数据转换前反转布尔掩码。
