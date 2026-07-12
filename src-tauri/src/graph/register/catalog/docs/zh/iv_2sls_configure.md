# IV:2SLS Configure

与 **OLS & WLS Configure** 选项相同，默认行为对齐 Stata `ivregress 2sls`（VCE 不做 $n/(n-k)$ 小样本修正；报告 Wald/z）。

## 输入

| Pin | 说明 |
|-----|------|
| **Constant** | 是否包含截距 |
| **VCE** | 任意 **VCE:** 常量或 OLS 配置节点（**Cluster**、**HAC**、**Newey** 等） |
| **Time** | 时间索引；选择 HAC / Newey 类 VCE 时需要 |

## 输出

| Pin | 说明 |
|-----|------|
| **Config** | **OLSConfigure** 句柄 |

**Config** → **IV:2SLS Summary** / **IV:LIML Summary** 可选 **Config** pin。
