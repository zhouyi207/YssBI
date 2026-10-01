# ATE

Extracts the average treatment effect from a fitted treatment-effect result.

$$
ATE=E[Y(1)-Y(0)].
$$

$Y(1),Y(0)$ are potential outcomes; $D$ is observed treatment. The target is the full observed covariate population, including both treatment groups.

## Input

Connect the typed `result` from PSM, IPW, regression adjustment or AIPW to `effects`. No raw outcome series or model parameters are accepted. This node does not fit a new estimator; identification, matching, weights and uncertainty come from the upstream method. It does not accept arbitrary reports, RDD local effects or synthetic-control time gaps.

## Output and interpretation

`result` contains `estimand`, upstream `method`, full-sample `observations`, `target_observations`, `effect`, `inference` and `bootstrap_replications`. The effect preserves the upstream estimate, standard error, statistic, p-value and confidence interval. Null uncertainty fields remain null.

If upstream bootstrap inference was enabled, the test remains $H_0:\theta=0$ versus $H_1:\theta\ne0$, using normal $z=\hat\theta/SE$, two-sided p-value and 95% normal interval. PSM results have point estimates only. No extra test, resampling or population reweighting takes place.

Interpret the selected effect under the assumptions of the original estimator. ATE and ATT can differ when effects vary across covariates; they should not be interchanged.
