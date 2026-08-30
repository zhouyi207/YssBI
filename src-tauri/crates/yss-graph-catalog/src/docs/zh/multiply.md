# Multiply (×)

逐元素乘法，标量可广播：

$$
\text{Result} = A \times B
$$

任一输入为 `DataSeries` 时，双方提升为 `DataSeries<Float64>`；标量广播到序列长度。纯标量输出 `Float64`。

## 用法

用标量缩放整条序列，或两条等长序列逐元素相乘（如构造交互项后再回归）。
