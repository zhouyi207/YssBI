# IV:2SLS Summary

两阶段最小二乘（Stata `ivregress 2sls`）：

$$
Y = X_{\mathrm{exog}}\beta_1 + X_{\mathrm{endog}}\beta_2 + \varepsilon,\quad
X_{\mathrm{endog}} = Z\pi + X_{\mathrm{exog}}\gamma + u
$$

## 输入

- **Y** — 因变量
- **X:exogs** — 外生自变量（可重复 `DataSeries`）
- **X:endog** — 内生自变量（`DataFrame`，每列一个内生变量）
- **x_instruments** — 工具变量（`DataFrame`，含排除与包含工具）
- 可选 **Config**（**IV:2SLS Configure**）、可选 **Time**

## 输出

**Result** + IV 2SLS 报告窗口（一阶段、过度识别、Stock–Yogo 等）。
