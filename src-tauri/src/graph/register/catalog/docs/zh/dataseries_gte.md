# DataSeries Greater Equal (>=)

逐元素大于等于比较：$\text{Result}_i = (\text{Series}_i \geq \text{Value}_i)$。

**Value** 可为数值标量或等长数值 **DataSeries**；输出 **Boolean** `DataSeries`。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataSeries** | 输入 | 左操作数序列 |
| **Value** | 输入 | `Float64` / `Int64` 标量，或等长数值 **DataSeries** |
| **Result** | 输出 | 逐元素比较结果，`DataSeries<Boolean>` |

## 用法

连接两条等长数值 **DataSeries** 可逐元素比较；或连接标量阈值做广播比较。`Boolean` / `String` 类型不支持 `>=`。
