# WLS (Weighted Least Squares)

Fits a linear model with observation weights $w_i > 0$:

$$
\hat\beta_{\mathrm{WLS}} = (X' W X)^{-1} X' W Y, \quad W = \mathrm{diag}(w_1,\ldots,w_n)
$$

## Inputs

Same as **OLS**, plus **Weights** — a `Float64` `DataSeries` with the same length as **Y** (must be positive).

Optional **Config** from **OLS & WLS Configure** (constant, VCE). **Time** can be set on the node or via config.

## Outputs

- **Model** — **OLSModel** handle (compatible with **Predict**)
- **Fitted** / **Residuals** — in-sample series

Use **WLS Summary** to open the full regression report window.
