# WLS（加权最小二乘）

带观测权重 $w_i > 0$ 的线性模型：

$$
\hat\beta_{\mathrm{WLS}} = (X' W X)^{-1} X' W Y, \quad W = \mathrm{diag}(w_1,\ldots,w_n)
$$

## 输入

与 **OLS** 相同，另需 **Weights** — 与 **Y** 等长的正数 `Float64` `DataSeries`。

可选 **Config**（来自 **OLS & WLS Configure**：截距、VCE）。**Time** 可在节点或 Config 中设置。

## 输出

- **Model** — **OLSModel** 句柄（兼容 **Predict**）
- **Fitted** / **Residuals** — 样本内序列

需要完整报告窗口时使用 **WLS Summary**。
