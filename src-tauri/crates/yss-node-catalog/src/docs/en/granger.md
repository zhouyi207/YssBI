# VAR Granger causality tests

Connect a VAR Fit `model` to test whether other variables' histories add predictive information for an equation. There are no parameters; variables, lags, estimates and covariance come from the model.

## Individual-variable test

- **Null $H_0$:** all included lag coefficients of the tested variable are zero in the response equation.
- **Alternative $H_1$:** at least one of those coefficients is nonzero.

$$
W=b'V_b^{-1}b,\qquad
W\overset{H_0}{\approx}\chi_m^2.
$$

$b$ is the tested lag-coefficient vector and $V_b$ its covariance submatrix. $m$ counts actually included lags, not the maximum lag order.

## ALL joint test

- **Null $H_0$:** all included lag coefficients of every other variable are zero in the response equation.
- **Alternative $H_1$:** at least one of those coefficients is nonzero.

Use the same Wald formula with $b$ containing all other variables' lag coefficients and $(K-1)m$ degrees of freedom, where $K$ is the variable count.

## Outputs and interpretation

`result` and `report` are identical. Each `vargranger` row contains `eq_name` (response), `excluded` (tested variable or `ALL`), `chi2`, `df` and `p_value`.

P-values use the corresponding chi-square upper tail. $p<\alpha$ rejects no additional predictive contribution from that variable or group. The direction is `excluded` → `eq_name`, not intervention-based causality. No multiplicity correction is applied; unit-root or cointegrated series require appropriate model specification.

Method details: [VAR Granger tests](https://www.statsmodels.org/stable/vector_ar.html#granger-causality).
