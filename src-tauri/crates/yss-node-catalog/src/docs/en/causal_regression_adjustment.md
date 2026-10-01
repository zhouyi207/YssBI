# Regression adjustment

## Inputs and identification

Connect aligned numeric `response`, binary or numeric 0/1 `treatment`, and optional numeric repeated `predictors`.
False/true mean control/treated. Both treatment states must occur. Covariates must be measured before treatment. An empty covariate group fits intercept-only models. Missing, nonfinite, separated or singular inputs fail; no rows are silently removed.

ATE targets the observed covariate population; ATT targets the treated covariate population. Causal interpretation requires consistency, no interference, conditional exchangeability and overlap. These assumptions are not verified by a successful fit.

## Estimator and parameters

Fits separate OLS outcome regressions with intercepts in treated and control samples, using the same covariate list. Each subgroup design must have full column rank and positive residual degrees of freedom.

With predicted potential outcomes $\hat m_1(X_i)$ and $\hat m_0(X_i)$,

$$
\widehat{ATE}=\frac1n\sum_i[\hat m_1(X_i)-\hat m_0(X_i)],
\qquad
\widehat{ATT}=\frac1{n_1}\sum_iD_i[\hat m_1(X_i)-\hat m_0(X_i)].
$$

RA requires correctly specified outcome conditional means, in addition to the identification assumptions. No propensity model or numerical overlap bound is fitted. `potential_outcomes` contains control then treated predictions for every original row.

## Inference

Set `bootstrap_replications=0` (default) for point estimates, or an integer of at least 2 for inference. `seed=42` defaults to a nonnegative reproducible seed. Practical inference needs many more than two replications.
Each replication samples independent rows with replacement and refits every propensity/outcome stage. Any failed replication aborts the run; it is not discarded. Clusters and time dependence are not supported by this resampling scheme.

The bootstrap standard deviation supplies $SE$. The reported test is $H_0:\theta=0$ versus $H_1:\theta\ne0$, with $z=\hat\theta/SE$, asymptotic normal two-sided p-value, and $\hat\theta\pm1.96SE$ 95% interval. These are normal intervals, not percentile intervals; disabling bootstrap leaves all inference fields null.

## Result

`result` has the treatment-effect type and contains `ate`, `att`, sample counts, method and inference metadata. Each effect contains `estimate` and nullable `standard_error`, `statistic`, `p_value`, `confidence_interval`. Applicable propensity/prediction/matching arrays preserve original row order. Connect this result to the ATE or ATT node to extract the requested estimand.
