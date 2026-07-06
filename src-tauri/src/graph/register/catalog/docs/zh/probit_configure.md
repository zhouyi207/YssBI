# Probit Configure

为 **Probit** / **Probit Summary** 生成 **ProbitConfigure** 结构体。

## 输入

| Pin | 说明 |
|-----|------|
| **Constant** | 是否包含截距（未连接时默认 `true`） |

## 输出

| Pin | 说明 |
|-----|------|
| **Config** | **ProbitConfigure** 句柄 |

**Config** 接入 Probit 节点可选 **Config** pin。未连接时使用节点内置默认。
