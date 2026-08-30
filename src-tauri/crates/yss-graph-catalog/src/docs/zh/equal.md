# Equal（==）

判断两值是否相等：

$$
\text{Result} = (A = B)
$$

比较两个标量 `Float64` 的值相等性。输出为单个 `Boolean`（非 `DataSeries`）。

## 用法

驱动 **Branch** 条件，或与 **And** / **Or** 组合。序列逐元素比较请用 **DataSeries** 专用比较节点。
