# Durbin–Watson residual serial-correlation diagnostic

Connect a time-ordered residual `series` with at least four non-null finite observations. There are no parameters; current row order is used.

## Hypotheses and formula

For the first-order error correlation coefficient $\rho$:

- **Null $H_0$:** $\rho=0$.
- **Alternative $H_1$:** $\rho\ne0$; directional questions may use $\rho>0$ or $\rho<0$. This node computes the statistic without selecting a direction.

$$
d=\frac{\sum_{t=2}^n(e_t-e_{t-1})^2}{\sum_{t=1}^n e_t^2}.
$$

$e_t$ are input residuals. Values near 2 suggest little first-order correlation; lower values suggest positive correlation and higher values negative correlation.

## Outputs and interpretation

`result` and `report` are identical and contain only `d`, without a p-value or critical value. The null distribution depends on the original regression design, so `d` alone does not establish significance. Classical DW inference is unsuitable for direct use with lagged dependent variables; consider Breusch–Godfrey.

All-zero residuals currently return `d=2` as a degenerate convention, not a valid test pass.

Method details: [Durbin–Watson](https://www.statsmodels.org/stable/generated/statsmodels.stats.stattools.durbin_watson.html).
