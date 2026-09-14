# OLS 普通最小二乘回归

对因变量 $Y$ 与自变量 $X_1,\ldots,X_k$ 拟合线性模型：

$$
Y = \beta_0 + \beta_1 X_1 + \cdots + \beta_k X_k + \varepsilon
$$

估计量：

$$
\hat{\beta} = (X'X)^{-1} X'Y
$$

## 用法

连接 **Y** 与至少一个 **X** 自变量后执行图即可。**Model** 输出可复用的 **OLSModel** 句柄，供下游 **Predict** 等节点使用；**Fitted** / **Residuals** 为样本内拟合值与残差（`DataSeries<Float64>`）。

在节点的 **Detail → 模型配置** 中设置常数项和协方差估计。选择 HAC、Newey-West 或固定方差尺度时，同一面板会显示对应的附加设置。

若需要完整回归报告窗口及 **OLSResult** 结构体，请使用 **OLS Summary** 节点。
