# VAR Summary

Vector Autoregression VAR($p$) (Stata `varbasic`):

$$
Y_t = A_1 Y_{t-1} + \cdots + A_p Y_{t-p} + B X_t + u_t
$$

## Inputs

- **Variables** — multivariate endogenous series (`DataFrame`)
- **Lags** — lag order $p$
- Optional **Exog** — contemporaneous exogenous `DataFrame` (same row count as Variables)
- Listwise deletion for missing / non-finite values

## Output

**Result** + report window: coefficients, stability, Granger, orthogonalized IRF (OIRF), FEVD.
