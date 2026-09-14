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

## 用法

在筛选或其他下游数据转换前合并多个布尔掩码。将 **Equal** 等比较结果接入 **A**、**B**。
