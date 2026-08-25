# Standardize DataSeries

对 **Float64** **DataSeries** 做 z-score 标准化，按样本均值与标准差缩放：

$$
z = \frac{x - \mu}{\sigma}
$$

节点同时输出 **Transform** 句柄，便于后续还原到原始量纲。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataSeries** | 输入 | 待标准化的数值序列 |
| **Standardized** | 输出 | 标准化后的 **DataSeries**\<Float64\> |
| **Transform** | 输出 | 保存 $\mu$、$\sigma$ 的 `StandardizeTransform1D` 句柄 |

## 用法

连接 **Float64** **DataSeries** 并执行图，将 **Standardized** 接到模型或后续变换。**Transform** 需保留以便还原量纲——与标准化数据一并接入 **Inverse Standardize DataSeries**。
