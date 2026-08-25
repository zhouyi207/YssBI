# DataSeries Not Equal (!=)

逐元素不等比较：$\text{Result}_i = (\text{Series}_i \neq \text{Value}_i)$。

支持数值、布尔与字符串；输出 **Boolean** `DataSeries`。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataSeries** | 输入 | 左操作数序列 |
| **Value** | 输入 | 标量（`Float64` / `Int64` / `Boolean` / `String`）或等长 **DataSeries** |
| **Result** | 输出 | 逐元素比较结果，`DataSeries<Boolean>` |

## 用法

连接 **DataSeries** 与 **Value**。两处均为 **DataSeries** 时长度须一致。常用于筛选异常值或与常量阈值比较。
