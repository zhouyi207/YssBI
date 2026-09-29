# Durbin–Watson residual serial-correlation diagnostic

Connect a time-ordered residual `series` with at least four non-null finite observations. There are no parameters. Current row order is used without sorting or interpreting time intervals.

## Hypotheses and statistic

In the classical first-order error model $u_t=\rho u_{t-1}+\varepsilon_t$:

- **Null $H_0$:** $\rho=0$, no first-order serial correlation.
- **Alternative $H_1$:** a study may target $\rho>0$, $\rho<0$ or $\rho\ne0$. This node only computes the statistic; it does not select a direction or make a rejection decision.

For input residuals $e_1,\ldots,e_n$,

$$
d=\frac{\sum_{t=2}^n(e_t-e_{t-1})^2}{\sum_{t=1}^n e_t^2}.
$$

Residuals are not demeaned. For nondegenerate data, $0\leq d\leq4$, and under common conditions $d\approx2(1-\hat\rho_1)$. Values near 2 suggest little first-order correlation; lower values suggest positive correlation and higher values negative correlation. This approximation is not a significance rule. See [statsmodels Durbin–Watson](https://www.statsmodels.org/stable/generated/statsmodels.stats.stattools.durbin_watson.html).

## Outputs and inference limits

`result` and `report` are identical and contain `d`. **There is no p-value, degrees of freedom, critical value or pass/fail decision.** The null distribution depends on the original regression design, not a fixed t, F or chi-square distribution; this series-only node does not receive that design.

For example, $d=0.8$ motivates investigating positive serial correlation but does not alone establish significance at 5%. Classical DW inference has model restrictions, especially for regressions containing lagged dependent variables. Consider model-based Breusch–Godfrey for higher-order serial correlation.

If all residuals are zero, the denominator vanishes and the current convention returns `d=2`; this is not a valid finding of no correlation. Nonzero constant residuals produce `d=0` and also require checking degeneracy first. Nulls, nonfinite values and nonfinite results from overflow fail.
