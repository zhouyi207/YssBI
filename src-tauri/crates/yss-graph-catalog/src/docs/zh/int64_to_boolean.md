# Int64 to Boolean（整数转布尔）

将 `DataSeries<Int64>` cast 为 `DataSeries<Boolean>`：$0 \to \text{false}$，非零 $\to \text{true}$。

## 用法

将数值指示列转为布尔掩码，供 **Branch** 或筛选使用。零明确为 false。
