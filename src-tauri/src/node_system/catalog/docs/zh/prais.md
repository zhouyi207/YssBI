# Prais

Prais–Winsten / Cochrane–Orcutt AR(1) 误差修正回归：

$$
y_t = x_t'\beta + u_t,\quad u_t = \rho u_{t-1} + \varepsilon_t
$$

## 输入

- **Y**、**X** 自变量
- **Time** — 强烈建议提供（观测顺序）
- 可选 **Prais Configure**（**Transform**：`prais` 或 `corc`）

## 输出

- **Model** — **PraisModel**
- **Fitted** / **Residuals**

完整报告请用 **Prais Summary**。
