# Inverse Standardize DataSeries

使用 **Standardize DataSeries** 输出的 **Transform** 句柄，将已标准化的 **Float64** **DataSeries** 还原到原始量纲：

$$
x = z \cdot \sigma + \mu
$$

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataSeries** | 输入 | 标准化后的 **DataSeries**\<Float64\> |
| **Transform** | 输入 | 来自 **Standardize DataSeries** 的 `StandardizeTransform1D` |
| **Result** | 输出 | 还原到原始单位的序列 |

## 用法

**Transform** 须与产生该标准化序列的 **Standardize DataSeries** 节点配对连接。在 z-score 数据上完成预测或处理后，若下游节点或报告需要可解释的原尺度数值，可经本节点还原。
