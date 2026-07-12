# DF & ADF Summary

对 **Y** 在 **Constant**、**Trend**、**Lags** 组合网格上批量运行 DF/ADF，返回列表结果便于对比。

最大滞后按 Stata 默认：$\lfloor 12\,(T/100)^{1/4}\rfloor$。遍历 $(constant,trend)\in\{(0,0),(1,0),(1,1)\}$ 与 $lags=0\ldots max\_lags$。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **In** | 执行输入 | 控制流入口 |
| **Y** | 输入 | `DataSeries<Float64>` |
| **Result** | 输出 | `DFADFSummaryListResult` 结构体 |
| **Out** | 执行输出 | 控制流出口 |

## 用法

执行后发布 Summary 报告并在 Info 窗口展示列表；单一设定请用 **DF & ADF** 节点。样本不足的规格会跳过并写日志。
