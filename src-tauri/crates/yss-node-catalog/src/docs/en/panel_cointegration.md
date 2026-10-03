# Panel cointegration (Fisher–Engle–Granger)

Connect aligned finite numeric **Y**, 1–5 **X₁, X₂, …**, numeric **entity** IDs and integer period indices **time**, without missing values. At least two entities are required. Each entity must have consecutive, unique periods; sample lengths and start/end periods may differ. Input variables should be integrated of order one, I(1); the node does not establish integration order automatically.

**Parameters:** lags defaults to 1, a nonnegative integer, fixing the lagged differences in residual ADF regressions. regression defaults to constant; trend adds a linear trend to the cointegrating equation.

For each entity estimate $y_{it}=a_i+b_it+x_{it}'\beta_i+e_{it}$, with regression selecting the trend, then run a residual ADF without an intercept or trend. Individual p-values use Engle–Granger MacKinnon response surfaces with dimension equal to response plus predictor count and the first-stage deterministic specification. These are not ordinary residual-ADF p-values.

The null is no cointegration in every entity; the alternative is cointegration in at least some entities. For independent entities,

$$
P=-2\sum_{i=1}^{N}\log p_i \ \sim\ \chi^2_{2N}.
$$

A small p-value rejects the null without establishing cointegration everywhere or estimating a common cointegrating vector.

The only **result** contains method, deterministic, lags, observations, statistic, degrees_of_freedom, p_value and entity_tests. Entity rows retain residual ADF statistics, calibrated p-values, effective observations and cointegrating_coefficients ordered as intercept, optional trend, then predictors in input order. A zero individual p-value yields statistic null (the positive-infinite limit) and p_value 0.

Sufficient time observations, positive residual degrees of freedom and a full-rank cointegrating design are required. Exact deterministic relationships or degenerate residuals cannot produce a finite ADF statistic and fail. The method assumes cross-sectional independence; it does not implement Pedroni, Kao, Westerlund or bootstrap corrections. The chosen response direction affects finite-sample results.

See the [statsmodels Engle–Granger documentation](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.coint.html).
