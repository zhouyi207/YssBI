# DF / ADF

Connect a finite numeric series in time order. lags may be zero (DF) or positive (ADF); regression selects none, constant or trend.

The null hypothesis is a unit root. The single structured result includes the statistic, p-value, critical values, actual lag order and sample size. No fitted model is produced. Insufficient observations or failed estimation are errors.

The series input accepts one or more aligned series. Multiple tests retain per-series success/failure information and separate auxiliary-regression rows. Results include the lagged-level coefficient and SE, full auxiliary regression, regression specification and `useTDistribution`. This preserves the current estimator's drift/constant convention: inspect the explicit distribution flag; auxiliary-regression coefficient p-values are not substituted for the unit-root test p-value.
