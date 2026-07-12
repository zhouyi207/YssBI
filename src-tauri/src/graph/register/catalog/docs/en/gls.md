# GLS (Generalized Least Squares)

Fits a linear model when errors have a known covariance structure $\Sigma$ (n×n):

$$
\hat\beta_{\mathrm{GLS}} = (X' \Sigma^{-1} X)^{-1} X' \Sigma^{-1} Y
$$

## Inputs

- **Y**, one or more **X** regressors (same as OLS)
- **Sigma** — square `DataFrame` (n×n) of error covariance
- Optional **Config** from **GLS Configure**, optional **Time**

## Outputs

- **Model** — **OLSModel** handle (for **Predict**)
- **Fitted** / **Residuals**

Use **GLS Summary** for the full report window.
