# Propensity-score matching

## Inputs and identification

Connect aligned numeric `Y`, binary or numeric 0/1 `treatment`, and optional numeric repeated `X₁, X₂, …`.
False/true mean control/treated. Both treatment states must occur. Covariates must be measured before treatment. An empty covariate group fits intercept-only models. Missing, nonfinite, separated or singular inputs fail; no rows are silently removed.

ATE targets the observed covariate population; ATT targets the treated covariate population. Causal interpretation requires consistency, no interference, conditional exchangeability and overlap. These assumptions are not verified by a successful fit.

## Estimator and parameters

Estimates ATE and ATT by nearest-neighbour matching with replacement on fitted Logit propensity probabilities.

For each observation, find the opposite-treatment observation with minimum propensity distance. All exact nearest-distance ties receive equal weight. Let $\tilde Y_i$ be their mean outcome; the individual contrast is $Y_i-\tilde Y_i$ for treated rows and $\tilde Y_i-Y_i$ for controls. ATE averages all contrasts, while ATT averages treated contrasts.

`ps_caliper=0.2` is the maximum absolute probability distance, in (0,1]. Every observation must have a match within the caliper. Failure is explicit; no observations are silently trimmed. Matching uses probability distance, not log-odds or standardized distance.

Standard errors, p-values and confidence intervals are unavailable. Ordinary row bootstrap is not offered for this fixed-neighbour estimator. Matching uncertainty requires inference that accounts for estimated propensity scores and match reuse; a naive paired t-test is not provided. See [Abadie and Imbens, matching estimators](https://www.nber.org/papers/t0283).

`matched_outcomes`, `match_counts` and `maximum_match_distance` retain the actual matching facts. Ties may give more than one neighbour. Counts and matched outcomes follow original row order.

The propensity is $P(D=1\mid X)$ from Logit with an intercept. `ps_overlap=0.000001` must lie strictly between 0 and 0.5; every estimated score must lie in `[ps_overlap,1-ps_overlap]`. Scores are not clipped and observations are not trimmed. `max_iterations=500` is a positive integer; `tolerance=0.0000001` lies in [1e-12,0.01]. Nonconvergence is an error.

## Result

`result` has the treatment-effect type and contains `ate`, `att`, sample counts, method and inference metadata. Each effect contains `estimate` and nullable `standard_error`, `statistic`, `p_value`, `confidence_interval`. Applicable propensity/prediction/matching arrays preserve original row order. Connect this result to the ATE or ATT node to extract the requested estimand.
