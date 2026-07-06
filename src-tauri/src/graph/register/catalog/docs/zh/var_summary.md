# VAR Summary

向量自回归 VAR($p$)（Stata `varbasic`）：

$$
Y_t = A_1 Y_{t-1} + \cdots + A_p Y_{t-p} + B X_t + u_t
$$

## 输入

- **Variables** — 多变量内生序列（`DataFrame`）
- **Lags** — 滞后阶 $p$
- 可选 **Exog** — 同期外生 `DataFrame`（行数与 Variables 一致）
- 缺失/非有限值 listwise 删除

## 输出

**Result** + 报告窗口：系数、稳定性、Granger、正交化 IRF（OIRF）、FEVD。
