# Equal（==）

判断两值是否相等：

$$
\text{Result} = (A = B)
$$

比较两个标量 `Float64` 的值相等性。输出为单个 `Boolean`（非 `DataSeries`）。

## 输入

| Pin | 说明 |
|-----|------|
| **A**（可选） | 第一个 `Float64` 操作数 |
| **B**（可选） | 第二个 `Float64` 操作数 |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | **A** 与 **B** 相等为 `true`，否则 `false` |

## 用法

驱动 **Branch** 条件，或与 **And** / **Or** 组合。序列逐元素比较请用 **DataSeries** 专用比较节点。
