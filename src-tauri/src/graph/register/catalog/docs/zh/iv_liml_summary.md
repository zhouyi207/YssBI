# IV:LIML Summary

有限信息极大似然 IV 估计（Stata `ivregress liml`）。输入布局与 **IV:2SLS Summary** 相同。

## 输入

| Pin | 说明 |
|-----|------|
| **In** | 执行流入口 |
| **Y** | 因变量 |
| **X:exogs** | 外生自变量（可重复 `DataSeries`） |
| **X:endog** | 内生自变量（`DataFrame`，每列一个内生变量） |
| **x_instruments** | 工具变量（`DataFrame`） |
| **Config** | 可选 **IV:2SLS Configure** |
| **Time** | 可选时间索引（部分 VCE 需要） |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | **OLSResult** |
| **Out** | 执行流出口 |

运行后打开 LIML 报告窗口。工具变量较弱时常用 LIML（2SLS 有限样本偏差较大）。
