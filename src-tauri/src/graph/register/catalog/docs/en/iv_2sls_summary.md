# IV:2SLS Summary

Two-Stage Least Squares (Stata `ivregress 2sls`):

$$
Y = X_{\mathrm{exog}}\beta_1 + X_{\mathrm{endog}}\beta_2 + \varepsilon,\quad
X_{\mathrm{endog}} = Z\pi + X_{\mathrm{exog}}\gamma + u
$$

## Inputs

- **Y** — dependent variable
- **X:exogs** — exogenous regressors (repeatable `DataSeries`)
- **X:endog** — endogenous regressors (`DataFrame`, one column per endogenous variable)
- **x_instruments** — instruments (`DataFrame`, excluded + included instruments)
- Optional **Config** from **IV:2SLS Configure**, optional **Time**

## Output

**Result** + IV 2SLS summary window (first stage, overid, Stock–Yogo, etc.).
