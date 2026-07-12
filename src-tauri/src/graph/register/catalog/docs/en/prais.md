# Prais

Prais–Winsten / Cochrane–Orcutt regression for AR(1) errors:

$$
y_t = x_t'\beta + u_t,\quad u_t = \rho u_{t-1} + \varepsilon_t
$$

## Inputs

- **Y**, **X** regressors
- **Time** — strongly recommended (observation order)
- Optional **Prais Configure** (**Transform**: `prais` or `corc`)

## Outputs

- **Model** — **PraisModel**
- **Fitted** / **Residuals**

Use **Prais Summary** for the full report.
