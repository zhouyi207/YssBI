# Logit Configure

为 **Logit** / **Logit Summary** 生成 **LogitConfigure** 结构体。

## 输入

| Pin | 说明 |
|-----|------|
| **Constant** | 是否包含截距（未连接时默认 `true`） |

## 输出

| Pin | 说明 |
|-----|------|
| **Config** | **LogitConfigure** 句柄 |

**Config** 接入 Logit 节点可选 **Config** pin。未连接时使用节点内置默认。
