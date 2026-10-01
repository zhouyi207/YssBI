# Dynamic panel (one-step difference GMM)

Connect aligned numeric **response**, optional strictly exogenous **predictors**, numeric **entity** IDs and integer period indices **time**. Values must be finite and complete. Rows are sorted by keys. A balanced panel is required: the same start/end periods for every entity, exactly one row per consecutive period, at least 4 periods and 3 entities, and more entities than instruments.

The model is $y_{it}=\rho y_{i,t-1}+x_{it}'\beta+\alpha_i+\varepsilon_{it}$. Differencing removes $\alpha_i$. One-step Arellano–Bond GMM estimates

$$
\hat\theta=(X'ZWZ'X)^{-1}X'ZWZ'\Delta y,\qquad
W=\left(\sum_i Z_i'H_iZ_i\right)^{-1}.
$$

$X$ contains the differenced lagged response and differenced predictors. $H_i$ has diagonal 2 and adjacent entries -1. Instruments are collapsed into one column per response lag; current strictly exogenous predictor differences instrument themselves. The difference equation has no additional intercept.

**Parameters:**

- max_instrument_lag defaults to 3, an integer of at least 2, and must be below the period count. Level instruments run from $y_{i,t-2}$ through this lag, with zero entries where a lag is not yet observed.
- covariance defaults to robust: entity moment-score sandwich, without a small-sample correction. nonrobust assumes homoskedastic, serially independent level errors; their variance is estimated as half the mean squared differenced residual.

The only **result** contains coefficients, parameter_names, inference (covariance, standard errors, z statistics, two-sided p-values and 95% normal intervals), instruments, entities, time_periods, observations, fitted, residuals and source_rows. Coefficient tests use a zero-coefficient null. The first two periods do not contribute equations. Fitted values/residuals are differenced quantities; source_rows gives each equation's current-period zero-based original row.

Identification requires serially uncorrelated level errors, valid lag-level instruments and strictly exogenous predictors. This entry does not supply system/two-step GMM, instruments for endogenous predictors, time dummies or AR/Sargan/Hansen tests. Coefficient significance does not validate instruments. Rank-deficient instruments or designs fail.

See the [Stata Arellano–Bond manual](https://www.stata.com/manuals/xtxtabond.pdf) for methodological context.
