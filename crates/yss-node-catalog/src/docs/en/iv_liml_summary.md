# IV LIML Summary

Connect the fitted `model` from IV LIML Fit. Configure selects model overview and coefficient inference by default. Enable `first_stage` or `overidentification` to compute those analyses from the same fitted sample and specification.

Outputs `result` containing the selected sections. No raw data or estimation parameters are required, and the LIML model is not refitted. Overidentification requires extra instruments and nonrobust covariance; unavailable statistics are reported as null.

Coefficient tests use $H_0:\beta_j=0$ against $H_1:\beta_j\ne0$ with statistic $\hat\beta_j/\operatorname{SE}(\hat\beta_j)$ and the selected covariance. The reference is standard normal by default, or Student-t with $n-k$ degrees when `small=true`; the same reference supplies the 95% interval critical value. Here $n$ is observations and $k$ counts all estimated coefficients, including the intercept.

The overall test has $H_0:\beta_s=0$ against at least one nonzero slope and statistic $W=\hat\beta_s^{\mathsf T}V_s^{-1}\hat\beta_s$, where $V_s$ is the slope covariance and $q$ is the number of tested coefficients, excluding an estimated intercept. By default $W$ uses $\chi^2(q)$; with `small=true`, $F=W/q$ uses $F(q,n-k)$. `model.statistics.modelTest` records `distribution` (`chiSquared` or `f`), `statistic`, `pValue` and either `df` or `dfNumerator`/`dfDenominator`. Small p-values reject the stated null.

Reports retain actual coefficient/instrument names and show structural and first-stage equations, named first-stage coefficients and weak-instrument critical values. Unavailable identification or covariance-dependent tests have explicit reasons. Optional `hypothesis_test`/`hypothesis` tests arbitrary independent coefficient restrictions: z/χ² by default, t/F with `small=true`, using the selected model covariance.
