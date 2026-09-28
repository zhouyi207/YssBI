# Prais

Prais–Winsten / Cochrane–Orcutt regression for AR(1) errors:

$$
y_t = x_t'\beta + u_t,\quad u_t = \rho u_{t-1} + \varepsilon_t
$$

Connect response and ordered predictors in time order; all values must be finite and aligned. Select transform (prais_winsten or cochrane_orcutt), intercept, maximum iterations (1–10000) and positive tolerance. Defaults are Prais–Winsten, an intercept, 100 iterations and tolerance 1e-6. Exhausted iterations are an error.

Outputs model, fitted values and residuals in original observation units. Prais Summary reads the fitted coefficients, inference, rho and iteration statistics without fitting again.
