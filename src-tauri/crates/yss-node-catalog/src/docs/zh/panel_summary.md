# Panel Summary

面板数据回归（类似 Stata `xtset` + `xtreg` 系列）。需要 **Entity ID** 与 **Time ID**（`Categorical` 或 `Int64`，与 **Y** 等长）。

## 模型族（报告内）

- 固定效应（Within）
- LSDV
- 一阶差分
- 随机效应（RE）
- Hausman 检验（FE vs RE）
