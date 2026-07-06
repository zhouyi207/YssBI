# DataSeries Mean

计算数值 **DataSeries** 的算术均值，输出 **Float64** 标量。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataSeries** | 输入 | 数值序列 |
| **Mean** | 输出 | 样本均值（`Float64`） |

## 用法

连接 **Get DataSeries** 或其它数值序列。无法计算均值（如全 null）时节点报错。
