# WLS (Weighted Least Squares)

Fits a linear model with observation weights $w_i > 0$:

$$
\hat\beta_{\mathrm{WLS}} = (X' W X)^{-1} X' W Y, \quad W = \mathrm{diag}(w_1,\ldots,w_n)
$$
