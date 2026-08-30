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

可选 **Config** 接入 **OLS Configure**（常数项、WLS 权重、稳健 / 聚类 / HAC / Newey 等协方差）。**Time** 仅在协方差类型需要时间索引时使用。

若需要完整回归报告窗口及 **OLSResult** 结构体，请使用 **OLS Summary** 节点。
