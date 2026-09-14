# WLS（加权最小二乘）

带观测权重 $w_i > 0$ 的线性模型：

$$
\hat\beta_{\mathrm{WLS}} = (X' W X)^{-1} X' W Y, \quad W = \mathrm{diag}(w_1,\ldots,w_n)
$$
