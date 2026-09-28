# VEC 汇总

连接 VEC Fit 产生的 `model`。Configure 默认选择模型概览、系数推断和 `cointegration` 协整统计；开启 `serial_tests` 或 `stability` 后，基于已有模型计算残差 LM 或稳定性根。残差 LM 启用后使用 `serial_lags`（1–40，默认 2）。

`result` 和 `report` 只包含所选内容。不接收原始数据或估计参数，不重新拟合。
