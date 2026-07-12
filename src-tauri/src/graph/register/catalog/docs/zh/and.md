# And（&&）

两布尔值的逻辑与：

$$
\text{Result} = A \land B
$$

| $A$ | $B$ | 结果 |
|-----|-----|------|
| false | false | false |
| false | true | false |
| true | false | false |
| true | true | true |

## 输入

| Pin | 说明 |
|-----|------|
| **A**（可选） | 第一个 `Boolean` |
| **B**（可选） | 第二个 `Boolean` |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | `Boolean`：两者均为 true 时为 true |

## 用法

在 **Branch** 或 **Set Variable** 前合并多个条件。将 **Equal** 等比较结果接入 **A**、**B**。
