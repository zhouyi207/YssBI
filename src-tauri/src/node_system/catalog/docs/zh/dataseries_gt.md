# DataSeries Greater Than (>)

逐元素大于比较：$\text{Result}_i = (\text{Series}_i > \text{Value}_i)$。

**Value** 可为数值标量或等长数值 **DataSeries**；输出 **Boolean** `DataSeries`。

## 用法

连接两条等长数值 **DataSeries** 可逐元素比较；或连接标量阈值做广播比较。`Boolean` / `String` 类型不支持 `>`，请使用 **Equal** / **Not Equal** 节点。
