# OLS Summary

Uses the same inputs as the **OLS** node. After estimation it:

1. Emits a full **OLSResult** struct
2. Opens the **OLS Summary** result window (coefficients, diagnostics, formulas)

## Model

$$
Y = X\beta + \varepsilon,\quad \hat{\beta} = (X'X)^{-1}X'Y
$$

Residual sum of squares:

$$
RSS = \sum_{i=1}^{n}(y_i - \hat{y}_i)^2
$$

Use the **OLS** node instead if you only need a reusable **Model** handle without opening the summary window.
