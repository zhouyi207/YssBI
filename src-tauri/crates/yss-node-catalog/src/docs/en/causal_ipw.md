# Inverse-probability weighting

## Inputs and identification

Connect aligned numeric `response`, binary or numeric 0/1 `treatment`, and optional numeric repeated `predictors`.
False/true mean control/treated. Both treatment states must occur. Covariates must be measured before treatment. An empty covariate group fits intercept-only models. Missing, nonfinite, separated or singular inputs fail; no rows are silently removed.

ATE targets the observed covariate population; ATT targets the treated covariate population. Causal interpretation requires consistency, no interference, conditional exchangeability and overlap. These assumptions are not verified by a successful fit.

## Estimator and parameters

Uses a Logit propensity model and normalized Hájek weights. With treatment $D_i$, propensity $e_i$ and observed outcome $Y_i$,

$$
\widehat{ATE}=
\frac{\sum_i D_iY_i/e_i}{\sum_iD_i/e_i}
-\frac{\sum_i(1-D_i)Y_i/(1-e_i)}{\sum_i(1-D_i)/(1-e_i)}.
$$

ATT subtracts the control outcome mean weighted by $e_i/(1-e_i)$ from the observed treated mean. Weights are normalized separately within treatment groups. This is not an unnormalized Horvitz–Thompson estimate or a survey-weight estimator.

The propensity is $P(D=1\mid X)$ from Logit with an intercept. `ps_overlap=0.000001` must lie strictly between 0 and 0.5; every estimated score must lie in `[ps_overlap,1-ps_overlap]`. Scores are not clipped and observations are not trimmed. `max_iterations=500` is a positive integer; `tolerance=0.0000001` lies in [1e-12,0.01]. Nonconvergence is an error.

## Inference

Set `bootstrap_replications=0` (default) for point estimates, or an integer of at least 2 for inference. `seed=42` defaults to a nonnegative reproducible seed. Practical inference needs many more than two replications.
Each replication samples independent rows with replacement and refits every propensity/outcome stage. Any failed replication aborts the run; it is not discarded. Clusters and time dependence are not supported by this resampling scheme.

The bootstrap standard deviation supplies $SE$. The reported test is $H_0:\theta=0$ versus $H_1:\theta\ne0$, with $z=\hat\theta/SE$, asymptotic normal two-sided p-value, and $\hat\theta\pm1.96SE$ 95% interval. These are normal intervals, not percentile intervals; disabling bootstrap leaves all inference fields null.

## Result

`result` has the treatment-effect type and contains `ate`, `att`, sample counts, method and inference metadata. Each effect contains `estimate` and nullable `standard_error`, `statistic`, `p_value`, `confidence_interval`. Applicable propensity/prediction/matching arrays preserve original row order. Connect this result to the ATE or ATT node to extract the requested estimand.
