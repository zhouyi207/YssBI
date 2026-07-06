# GLS Configure

为 **GLS** / **GLS Summary** 生成 **GLSConfigure** 结构体。

## 输入

| Pin | 说明 |
|-----|------|
| **Constant** | 是否包含截距（未连接时默认 `true`） |
| **Time** | 可选时间索引（`Int64` 或 `Date` 的 `DataSeries`），用于诊断与报告元数据 |

## 输出

| Pin | 说明 |
|-----|------|
| **Config** | **GLSConfigure** 句柄 |

**Config** 接入 **GLS** / **GLS Summary** 可选 **Config** pin。未连接时使用节点内置默认（含截距）。
