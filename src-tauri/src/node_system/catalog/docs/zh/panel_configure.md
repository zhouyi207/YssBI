# Panel Configure

为 **Panel Summary** 与 **Panel DID (TWFE)** 生成 **PanelConfigure** 结构体。

## 输入

| Pin | 默认 | 说明 |
|-----|------|------|
| **Constant** | `true` | Within/LSDV 等设定中的截距 |
| **VCE** | 按 entity 聚类 | **VCE: NonRobust** / **HC0–HC3** / **VCE: Cluster (by Entity)** |

## 输出

| Pin | 说明 |
|-----|------|
| **Config** | **PanelConfigure** 句柄 |

**Config** → **Panel Summary** / **Panel DID** 可选 **Config** pin。DID 相关选项（平行趋势、安慰剂）保存在 config 中供报告扩展使用。
