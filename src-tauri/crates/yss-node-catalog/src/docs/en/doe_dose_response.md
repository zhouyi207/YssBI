# Dose response (four-parameter log-logistic)

Connect aligned nonnegative numeric **dose** and continuous numeric **Y**. Each row is an independent observation; zero dose is evaluated at the continuous limit.
Fits equal-weight nonlinear least squares: `c + (d−c)/(1+(dose/ED50)^Hill)`.

- c is the infinite-dose asymptote (`asymptote_infinite`); d is the zero-dose asymptote (`asymptote_zero`).
- Hill > 0 controls steepness. The relative values of c and d determine increasing or decreasing direction.
- ED50 > 0 is the dose at the midpoint of the two asymptotes, not half the observed maximum.

log(Hill) and log(ED50) are fitted to ensure positivity; reported log(ED50) and ED50 use the input dose unit.
`max_iterations` defaults to 500 and `tolerance` to 1e-7. Starts use dose endpoints and the median positive dose.
More than four observations and a full-rank final four-parameter Jacobian are required. Constant responses, inadequate dose information, nonconvergence and nonfinite results fail explicitly.

**result** gives the four coefficients, local Jacobian covariance, approximate t-based 95% intervals, RSS/RMSE and likelihood criteria.
Arbitrary zero tests on log(Hill) or log(ED50) are omitted. **details** reports Hill and ED50 with delta-method standard errors and exponentiated log-parameter intervals; unrepresentable intervals are absent.
It also identifies curve direction and whether ED50 lies within the observed dose range.
**observations** contains every observation number, response, fitted value and residual, without a 512-row cap.

This is a single continuous-response curve with independent homoskedastic errors, not binary logistic regression, toxicity-risk prediction or dose-response meta-analysis.
Data without adequate coverage of both plateaus may not identify ED50 reliably. Profile-likelihood intervals, weighted fits and shared parameters across curves are not provided.

[Reference: drc four-parameter log-logistic model](https://doseresponse.github.io/drc/reference/LL.4.html)
