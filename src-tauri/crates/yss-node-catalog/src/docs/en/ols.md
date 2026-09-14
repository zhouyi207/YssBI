# Ordinary Least Squares (OLS)

Fits a linear model for dependent variable $Y$ and regressors $X_1,\ldots,X_k$:

$$
Y = \beta_0 + \beta_1 X_1 + \cdots + \beta_k X_k + \varepsilon
$$

The OLS estimator is:

$$
\hat{\beta} = (X'X)^{-1} X'Y
$$

## Usage

Connect **Y** and one or more **X** regressors, then run the graph. The node emits a fitted **OLSModel** handle on **Model** for downstream **Predict** nodes; **Fitted** and **Residuals** are the in-sample fitted values and residuals as `DataSeries<Float64>`.

Set the intercept and covariance estimator in the node's **Detail → Model configuration** panel. HAC, Newey-West, and fixed scale display their additional settings in the same panel.

Use **OLS Summary** instead if you want the full regression report window and an **OLSResult** struct rather than a reusable model handle.
