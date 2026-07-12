# GLS（广义最小二乘）

在误差协方差结构 $\Sigma$（n×n）已知时拟合线性模型：

$$
\hat\beta_{\mathrm{GLS}} = (X' \Sigma^{-1} X)^{-1} X' \Sigma^{-1} Y
$$

## 输入

- **Y**、一个或多个 **X** 自变量（同 OLS）
- **Sigma** — n×n 方阵 `DataFrame`（误差协方差）
- 可选 **Config**（**GLS Configure**）、可选 **Time**

## 输出

- **Model** — **OLSModel** 句柄（供 **Predict**）
- **Fitted** / **Residuals**

完整报告窗口请用 **GLS Summary**。
