# Or（||）

两布尔值的逻辑或：

$$
\text{Result} = A \lor B
$$

| $A$ | $B$ | 结果 |
|-----|-----|------|
| false | false | false |
| false | true | true |
| true | false | true |
| true | true | true |

## 输入

| Pin | 说明 |
|-----|------|
| **A**（可选） | 第一个 `Boolean` |
| **B**（可选） | 第二个 `Boolean` |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | `Boolean`：任一输入为 true 则为 true |

## 用法

表示“满足其一即可”（如阈值或标志位）。将比较节点结果接入 **A**、**B**。
