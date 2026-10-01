# KPSS stationarity test

Connect a finite numeric `series` in equally spaced time order, with at least three observations and no missing values. `ts_deterministic=constant` tests level stationarity; `trend` tests trend stationarity after removing an intercept and linear trend. `ts_bandwidth=4` is a nonnegative Newey–West truncation lag strictly below the observation count. There is no fixed row-count ceiling.

$H_0$: the series is stationary around the selected level/trend. $H_1$: it contains a stochastic trend. After removing deterministic terms by OLS, define residual partial sums $S_t=\sum_{i=1}^t e_i$:

$$
\mathrm{KPSS}=\frac{\sum_{t=1}^n S_t^2}{n^2\widehat\lambda^2}.
$$

$\widehat\lambda^2$ is the Bartlett/Newey–West long-run residual variance and must be positive. The reference distribution is the appropriate KPSS asymptotic distribution, not chi-square. Large statistics reject stationarity.

`critical_values` lists the 10%, 5%, 2.5% and 1% critical values. `p_value` interpolates their published table and is limited to [0.01,0.10]. Interpret it together with `p_value_kind`: `less_than` means the true tail is below 0.01; `greater_than` means it is above 0.10; `interpolated` means table interpolation, including exact boundary values. A boundary value must not be treated as an exact tail probability. `statistic`, `long_run_variance`, `observations`, `effective_observations`, `bandwidth` and `deterministic` complete the report. KPSS and PP/ADF have opposite null hypotheses and can be used together to investigate stationarity.

[KPSS reference](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.kpss.html)
