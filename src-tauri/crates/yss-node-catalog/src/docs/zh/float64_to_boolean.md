# Float64 to Boolean（浮点转布尔）

将 `DataSeries<Float64>` cast 为 `DataSeries<Boolean>`：$0 \to \text{false}$，非零 $\to \text{true}$。

## 用法

由连续得分构造布尔掩码（如概率 $> 0$）。NaN 通常为 null，而非 true/false。
