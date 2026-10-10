# VAR Summary

连接 VAR Fit 产生的 `model`。Configure 默认选择模型概览和系数推断；可开启 `lag_exclusion`、`serial_tests` 或 `stability` 计算滞后排除、残差 LM 或稳定性分析。残差 LM 启用后使用 `serial_lags`（1–40，默认 2）。

`result` 只包含所选内容。可选分析复用已有拟合结果，不重新拟合。Granger、IRF 和 FEVD 使用各自的独立节点。
